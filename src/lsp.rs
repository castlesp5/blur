use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

fn read_message<R: BufRead>(r: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return Ok(None); // server closed
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                len = v.trim().parse::<usize>().ok();
            }
        }
    }
    let mut body = vec![0u8; len.ok_or(io::ErrorKind::InvalidData)?];
    r.read_exact(&mut body)?;
    Ok(Some(body))
}

fn write_message<W: Write>(w: &mut W, body: &[u8]) -> io::Result<()> {
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(body);
    w.write_all(&out)?;
    w.flush()
}

fn path_to_uri(p: &Path) -> String {
    let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mut s = String::from("file://");
    for b in abs.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                s.push(b as char)
            }
            _ => s.push_str(&format!("%{:02X}", b)),
        }
    }
    s
}

fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let b = uri.strip_prefix("file://")?.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            out.push(u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).ok()?, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

pub fn utf16_to_byte(line: &str, col: u32) -> usize {
    let mut units = 0;
    for (i, ch) in line.char_indices() {
        if units >= col {
            return i;
        }
        units += ch.len_utf16() as u32;
    }
    line.len()
}

// cli-cli-client

#[derive(Debug, Clone)]
pub struct Diag {
    pub start: (u32, u32), // (line, utf16 column)
    pub end: (u32, u32),
    pub severity: u8, // 1 error, 2 warning, 3 info, 4 hint
    pub message: String,
}

pub struct Lsp {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Option<Value>>, // None = server died
    next_id: i64,
    root_uri: String,
    docs: HashMap<PathBuf, (i64, String, String)>, // path, (version, language, text)
    diags: HashMap<PathBuf, Vec<Diag>>,
    pub generation: u64,
    pub ready: bool,
    pub alive: bool,
}

impl Lsp {
    pub fn start(cmd: &str, args: &[&str], root: &Path) -> io::Result<Self> {
        let log = File::create(std::env::temp_dir().join("lsp-stderr.log"))?;
        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log) // keep it quiet
            .spawn()?;
        let stdin = child.stdin.take().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            loop {
                match read_message(&mut stdout) {
                    Ok(Some(body)) => {
                        if let Ok(v) = serde_json::from_slice(&body) {
                            if tx.send(Some(v)).is_err() {
                                return;
                            }
                        }
                    }
                    _ => {
                        let _ = tx.send(None);
                        return;
                    }
                }
            }
        });

        let mut lsp = Lsp {
            child,
            stdin,
            rx,
            next_id: 1,
            root_uri: path_to_uri(root),
            docs: HashMap::new(),
            diags: HashMap::new(),
            ready: false,
            alive: true,
            generation: 0,
        };
        let uri = lsp.root_uri.clone();
        lsp.send(
            "initialize",
            Some(json!({
                "processId": std::process::id(),
                "rootUri": uri,
                "workspaceFolders": [{ "uri": uri, "name": "root" }],
                "capabilities": {
                    "textDocument": { "publishDiagnostics": {} },
                    "workspace": { "configuration": true, "workspaceFolders": true },
                    "window": { "workDoneProgress": true }
                }
            })),
            true,
        );
        Ok(lsp)
    }

    fn send(&mut self, method: &str, params: Option<Value>, is_request: bool) {
        let mut m = json!({ "jsonrpc": "2.0", "method": method });
        if is_request {
            m["id"] = json!(self.next_id);
            self.next_id += 1;
        }
        if let Some(p) = params {
            m["params"] = p;
        }
        let _ = write_message(&mut self.stdin, m.to_string().as_bytes());
    }

    fn reply(&mut self, id: Value, result: Value) {
        let m = json!({ "jsonrpc": "2.0", "id": id, "result": result });
        let _ = write_message(&mut self.stdin, m.to_string().as_bytes());
    }

    pub fn open(&mut self, path: &Path, lang: &str, text: &str) {
        self.docs
            .insert(path.to_path_buf(), (1, lang.into(), text.into()));
        if self.ready {
            self.send_open(path);
        }
    }

    /// ~call me maybe-e-e-eeeh~  (seriously: call when text changed)
    pub fn change(&mut self, path: &Path, text: &str) {
        let Some(doc) = self.docs.get_mut(path) else {
            return;
        };
        doc.0 += 1; // version must rise!
        doc.2 = text.into();
        let version = doc.0;
        if self.ready {
            self.send(
                "textDocument/didChange",
                Some(json!({
                    "textDocument": { "uri": path_to_uri(path), "version": version },
                    "contentChanges": [{ "text": text }]
                })),
                false,
            );
        }
    }

    fn send_open(&mut self, path: &Path) {
        let Some((version, lang, text)) = self.docs.get(path).cloned() else {
            return;
        };
        self.send(
            "textDocument/didOpen",
            Some(json!({
                "textDocument": {
                    "uri": path_to_uri(path), "languageId": lang, "version": version, "text": text
                }
            })),
            false,
        );
    }

    /// ~call me maybe-e-eh maybe not-t-t~ (once per frame)
    pub fn poll(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(Some(v)) => self.handle(v),
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.alive = false;
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => return,
            }
        }
    }

    pub fn diagnostics(&self, path: &Path) -> &[Diag] {
        self.diags.get(path).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn handle(&mut self, v: Value) {
        let id = v.get("id").cloned();
        let method = v.get("method").and_then(|m| m.as_str()).map(String::from);
        let params = v.get("params").cloned().unwrap_or(Value::Null);
        match (id, method) {
            (Some(_), None) if !self.ready => {
                self.send("initialized", Some(json!({})), false);
                self.ready = true;
                for p in self.docs.keys().cloned().collect::<Vec<_>>() {
                    self.send_open(&p);
                }
            }
            (Some(id), Some(m)) => {
                let n = params["items"].as_array().map_or(0, |a| a.len());
                let result = if m == "workspace/configuration" {
                    json!(vec![Value::Null; n])
                } else {
                    Value::Null
                };
                self.reply(id, result);
            }
            (None, Some(m)) if m == "textDocument/publishDiagnostics" => {
                let Some(path) = params["uri"].as_str().and_then(uri_to_path) else {
                    return;
                };
                let num = |v: &Value| v.as_u64().unwrap_or(0) as u32;
                let list = params["diagnostics"].as_array().map_or(vec![], |a| {
                    a.iter()
                        .map(|d| Diag {
                            start: (
                                num(&d["range"]["start"]["line"]),
                                num(&d["range"]["start"]["character"]),
                            ),
                            end: (
                                num(&d["range"]["end"]["line"]),
                                num(&d["range"]["end"]["character"]),
                            ),
                            severity: d["severity"].as_u64().unwrap_or(1) as u8,
                            message: d["message"].as_str().unwrap_or("").to_string(),
                        })
                        .collect()
                });
                self.generation += 1;
                self.diags.insert(path, list);
            }
            _ => {}
        }
    }
}

impl Drop for Lsp {
    fn drop(&mut self) {
        if self.ready && self.alive {
            self.send("shutdown", None, true);
            let _ = self.rx.recv_timeout(Duration::from_millis(800));
            self.send("exit", None, false);
        }
        for _ in 0..20 {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
    }
}
