use crate::lsp::{Diag, Lsp};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// (language id, server command, file root)
fn server_for(path: &Path) -> Option<(&'static str, &'static str, &'static str)> {
    match path.extension()?.to_str()? {
        "rs" => Some(("rust", "rust-analyzer", "Cargo.toml")),
        _ => None,
    }
}

fn find_root(file: &Path, marker: &str) -> PathBuf {
    file.ancestors()
        .skip(1)
        .find(|d| d.join(marker).exists())
        .or(file.parent())
        .unwrap_or(file)
        .to_path_buf()
}

#[derive(Default)]
pub struct LspManager {
    servers: HashMap<&'static str, Lsp>, // language id, running server
}

impl LspManager {
    pub fn open(&mut self, path: &Path, text: &str) {
        let Ok(path) = std::fs::canonicalize(path) else {
            return;
        };
        let Some((lang, cmd, marker)) = server_for(&path) else {
            return;
        };
        if !self.servers.contains_key(lang) {
            match Lsp::start(cmd, &[], &find_root(&path, marker)) {
                Ok(s) => {
                    self.servers.insert(lang, s);
                }
                Err(_) => return, // server not installed - be ashamed hooman!
            }
        }
        self.servers.get_mut(lang).unwrap().open(&path, lang, text);
    }

    pub fn change(&mut self, path: &Path, text: &str) {
        let Ok(path) = std::fs::canonicalize(path) else {
            return;
        };
        let Some((lang, ..)) = server_for(&path) else {
            return;
        };
        if let Some(s) = self.servers.get_mut(lang) {
            s.change(&path, text);
        }
    }

    pub fn poll(&mut self) {
        for s in self.servers.values_mut() {
            s.poll();
        }
    }

    pub fn diagnostics(&self, path: &Path) -> &[Diag] {
        let Ok(path) = std::fs::canonicalize(path) else {
            return &[];
        };
        let Some((lang, ..)) = server_for(&path) else {
            return &[];
        };
        match self.servers.get(lang) {
            Some(s) => s.diagnostics(&path),
            None => &[],
        }
    }

    pub fn generation(&self) -> u64 {
        self.servers.values().map(|s| s.generation).sum()
    }
}
