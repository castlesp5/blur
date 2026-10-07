//! start screen shown when blur is launched with no file arguments.
//!
//! deliberately small: a logo, the few things you can do, and the files
//! you opened last. everything else lives one keypress away in the
//! editor.

use std::path::{Path, PathBuf};

use ratatui::Frame;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::Paragraph;

/// block letters in the figlet "big" style, embedded so blur keeps
/// working with no external tools installed.
pub const LOGO: &[&str] = &[
    "██████╗ ██╗     ██╗   ██╗██████╗",
    "██╔══██╗██║     ██║   ██║██╔══██╗",
    "██████╔╝██║     ██║   ██║██████╔╝",
    "██╔══██╗██║     ██║   ██║██╔══██╗",
    "██████╔╝███████╗╚██████╔╝██║  ██║",
    "╚═════╝ ╚══════╝ ╚═════╝ ╚═╝  ╚═╝",
];

const MAX_RECENTS: usize = 10;

/// what the start screen decided. `Prompt` just focuses the path input
/// and stays on the screen.
#[derive(Debug, PartialEq)]
pub enum Choice {
    Quit,
    New,
    Open(String),
    Prompt,
}

enum Row {
    Open,
    New,
    Recent(String),
    Quit,
}

impl Row {
    fn label(&self, icons: fn(&str) -> &'static str) -> String {
        match self {
            Row::Open => format!("{}{}", icons("open"), "open file"),
            Row::New => format!("{}{}", icons("new"), "new buffer"),
            Row::Quit => format!("{}{}", icons("quit"), "quit"),
            Row::Recent(p) => {
                let name = crate::short_of(p);
                let name = if name.is_empty() { p.clone() } else { name };
                format!("{}{}", crate::file_icon(p).0, name)
            }
        }
    }

    fn path(&self) -> Option<&str> {
        match self {
            Row::Recent(p) => Some(p),
            _ => None,
        }
    }
}

/// `BLUR_STATE_DIR` wins, which keeps tests and portable installs from
/// touching the real user state.
fn state_file() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("BLUR_STATE_DIR") {
        return Some(PathBuf::from(dir).join("recent"));
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("blur").join("recent"))
}

/// files opened before, newest first, duplicates removed
pub fn recent_files() -> Vec<String> {
    let Some(path) = state_file() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !out.iter().any(|p| p == line) {
            out.push(line.to_string());
        }
        if out.len() == MAX_RECENTS {
            break;
        }
    }
    out
}

/// remember a file, newest first. failures are ignored, a homescreen
/// should never break the editor because of a state file.
pub fn record_recent(path: &str) {
    // directories are not openable buffers, so never list them
    if path.is_empty() || !Path::new(path).is_file() {
        return;
    }
    // the same file can appear as a relative and an absolute path, so
    // compare the resolved forms as well as the text
    let key = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    let mut files = vec![path.to_string()];
    // keep existing entries only while they are still real files, so a
    // stale one is not written back on every open
    for f in recent_files() {
        if !Path::new(&f).is_file() {
            continue;
        }
        let same = f == path
            || std::fs::canonicalize(&f).unwrap_or_else(|_| PathBuf::from(&f)) == key;
        if !same {
            files.push(f);
        }
    }
    files.truncate(MAX_RECENTS);
    write_recents(&files);
}

fn write_recents(files: &[String]) {
    if let Some(file) = state_file() {
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let body = if files.is_empty() {
            String::new()
        } else {
            files.join("\n") + "\n"
        };
        let _ = std::fs::write(file, body);
    }
}

/// drop entries whose file is gone. editors do this so the recent list
/// never offers something that cannot be opened.
pub fn purge_missing() {
    let all = recent_files();
    let mut kept: Vec<String> = Vec::with_capacity(all.len());
    let mut seen: Vec<PathBuf> = Vec::with_capacity(all.len());
    let mut dropped = false;
    for p in all {
        // a file that is gone, or an entry that is not a file at all,
        // has no place in this list
        if !Path::new(&p).is_file() {
            dropped = true;
            continue;
        }
        let key = std::fs::canonicalize(&p).unwrap_or_else(|_| PathBuf::from(&p));
        if seen.contains(&key) {
            dropped = true;
            continue;
        }
        seen.push(key);
        kept.push(p);
    }
    if dropped {
        write_recents(&kept);
    }
}

pub struct Home {
    rows: Vec<Row>,
    sel: usize,
    input: Option<String>,
    screen_h: u16,
}

impl Home {
    pub fn new() -> Self {
        let mut rows = vec![Row::Open, Row::New];
        for path in recent_files() {
            rows.push(Row::Recent(path));
        }
        rows.push(Row::Quit);
        Self {
            rows,
            sel: 0,
            input: None,
            screen_h: 24,
        }
    }

    /// rows visible at once, for wheel scrolling on small terminals
    pub fn window(&self, height: usize) -> usize {
        height.saturating_sub(LOGO.len() + 8).max(3)
    }

    pub fn set_screen(&mut self, height: usize) {
        self.screen_h = height as u16;
    }

    pub fn top(&self, height: usize) -> usize {
        let rows = self.window(height);
        self.sel
            .saturating_sub(rows.saturating_sub(1) / 2)
            .min(self.rows.len().saturating_sub(rows))
    }

    /// where the path input sits on screen
    pub fn prompt_row(&self) -> u16 {
        let h = self.screen_h;
        h.saturating_sub(1)
    }

    pub fn prompt_col(&self) -> u16 {
        " path ".len() as u16
    }

    fn list_y(&self) -> u16 {
        // logo, blank, heading
        1 + LOGO.len() as u16 + 1 + 1
    }

    /// activate the selected row, or the current prompt contents
    pub fn activate(&mut self) -> Option<Choice> {
        if let Some(raw) = self.input.take() {
            let path = raw.trim().to_string();
            if path.is_empty() {
                // nothing typed, stay in the prompt instead of bailing out
                self.input = Some(String::new());
                return Some(Choice::Prompt);
            }
            return Some(Choice::Open(path));
        }
        match self.rows.get(self.sel)? {
            Row::Open => {
                self.input = Some(String::new());
                Some(Choice::Prompt)
            }
            Row::New => Some(Choice::New),
            Row::Quit => Some(Choice::Quit),
            Row::Recent(p) => Some(Choice::Open(p.clone())),
        }
    }

    /// mouse click, returns the choice when a row was hit
    pub fn click(&mut self, y: u16, height: usize) -> Option<Choice> {
        let top = self.top(height);
        let list = self.list_y();
        if y < list || y >= list + self.window(height) as u16 {
            return None;
        }
        let idx = top + (y - list) as usize;
        if idx >= self.rows.len() {
            return None;
        }
        self.sel = idx;
        self.activate()
    }

    /// highlight the first row, used when the screen is entered
    pub fn select_first(&mut self) {
        self.sel = 0;
    }

    pub fn move_sel(&mut self, delta: i32) {
        let n = self.rows.len() as i32;
        if n == 0 {
            return;
        }
        self.sel = ((self.sel as i32 + delta).rem_euclid(n)) as usize;
    }
    pub fn is_prompting(&self) -> bool {
        self.input.is_some()
    }

    pub fn cursor(&self) -> Option<usize> {
        self.input.as_ref().map(|s| s.chars().count())
    }

    pub fn prompt(&self) -> Option<&str> {
        self.input.as_deref()
    }

    /// start typing a path. returns true when the keystroke was consumed.
    pub fn type_char(&mut self, c: char) -> bool {
        match self.input.as_mut() {
            Some(buf) => {
                buf.push(c);
                true
            }
            None => {
                self.input = Some(c.to_string());
                true
            }
        }
    }

    pub fn backspace(&mut self) {
        if let Some(buf) = self.input.as_mut() {
            buf.pop();
            if buf.is_empty() {
                self.input = None;
            }
        }
    }

    pub fn cancel(&mut self) {
        self.input = None;
    }
}

impl Default for Home {
    fn default() -> Self {
        Self::new()
    }
}

fn icons(name: &str) -> &'static str {
    match name {
        "open" => " 󰈙 ",
        "new" => " 󰐒 ",
        _ => " 󰗼 ",
    }
}

pub fn draw(frame: &mut Frame, theme: &opaline::Theme, home: &Home, cursor: (u16, u16)) {
    let base: Color = Color::Reset;
    let text: Color = crate::tok(theme, "text.primary").into();
    let dim: Color = crate::tok(theme, "text.dim").into();
    let faint: Color = crate::tok(theme, "border.unfocused").into();
    let accent: Color = crate::tok(theme, "accent.deep").into();
    let second: Color = crate::hue(theme, "accent.secondary").into();

    let area = frame.area();
    let width = area.width as usize;
    let height = area.height as usize;

    let mut lines: Vec<Line> = Vec::new();

    // logo, centred, fading from accent to the secondary accent
    let logo_w = LOGO.iter().map(|l| l.chars().count()).max().unwrap_or(6);
    let left = ((width.saturating_sub(logo_w)) / 2) as u16;
    for (i, l) in LOGO.iter().enumerate() {
        let t = i as f32 / (LOGO.len().saturating_sub(1)) as f32;
        let c = mix(accent, second, t);
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(left as usize)),
            Span::styled(l.to_string(), Style::default().fg(c).bg(base).bold()),
        ]));
    }
    lines.push(Line::from(""));

    // the list
    let rows = home.window(height);
    let top = home.top(height);
    let heading = if home.rows.len() > 3 { "files" } else { " " };
    lines.push(Line::from(vec![
        Span::raw(" ".repeat(left as usize)),
        Span::styled(
            format!(" {} ", heading),
            Style::default().fg(faint).bg(base),
        ),
    ]));

    for (i, row) in home.rows.iter().enumerate().skip(top).take(rows) {
        let selected = i == home.sel;
        let label = row.label(icons);
        let pad = left.saturating_sub(1) as usize;
        let mut spans = vec![
            Span::raw(" ".repeat(pad)),
            Span::styled(" ", Style::default().bg(base)),
        ];
        if selected {
            spans.push(Span::styled(
                format!(" {} ", label.trim()),
                Style::default().fg(contrast(accent)).bg(accent).bold(),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {} ", label.trim()),
                Style::default().fg(dim).bg(base),
            ));
        }
        // show the full path for recent files, dimmed at the end of the row
        if let Some(path) = row.path()
            && !selected
        {
            let dir = match path.rsplit_once('/') {
                Some((d, _)) => {
                    if d.is_empty() {
                        String::new()
                    } else {
                        format!("  {}", d)
                    }
                }
                None => String::new(),
            };
            if !dir.is_empty() {
                spans.push(Span::styled(dir, Style::default().fg(faint).bg(base)));
            }
        }
        lines.push(Line::from(spans));
    }

    while lines.len() < height.saturating_sub(1) {
        lines.push(Line::from(""));
    }

    let mut body: Vec<Line> = lines;
    // prompt and hints replace the last rows
    let last = body.len();
    match home.input.as_deref() {
        Some(_) => {
            let text_in = home.prompt().unwrap_or("");
            let prefix = " path ";
            let mut spans = vec![
                Span::styled(
                    prefix.to_string(),
                    Style::default().fg(accent).bg(base).bold(),
                ),
                Span::styled(text_in.to_string(), Style::default().fg(text).bg(base)),
            ];
            spans.push(Span::styled(
                "█".to_string(),
                Style::default().fg(accent).bg(base),
            ));
            body[last - 1] = Line::from(spans);
            body[last - 2] = Line::from(Span::styled(
                "  enter open · esc cancel".to_string(),
                Style::default().fg(faint).bg(base),
            ));
        }
        None => {
            let hint = " j/k move · enter open · n new · q quit";
            let pad = (width.saturating_sub(hint.chars().count())) / 2;
            body[last - 1] = Line::from(vec![
                Span::raw(" ".repeat(pad)),
                Span::styled(hint.to_string(), Style::default().fg(faint).bg(base)),
            ]);
        }
    }

    let _ = cursor;
    frame.render_widget(Paragraph::new(Text::from(body)).bg(base), area);
}

/// readable text on a colored pill, without relying on a theme helper
fn contrast(bg: Color) -> Color {
    let rgb = match bg {
        Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
        _ => return Color::White,
    };
    let lin = |c: f32| {
        let c = c / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let lum = 0.2126 * lin(rgb.0) + 0.7152 * lin(rgb.1) + 0.0722 * lin(rgb.2);
    let on_black = (lum + 0.05) / 0.05;
    let on_white = 1.05 / (lum + 0.05);
    if on_black >= on_white {
        Color::Black
    } else {
        Color::White
    }
}

/// blend two colors, used for the logo fade
fn mix(a: Color, b: Color, t: f32) -> Color {
    let get = |c: Color| match c {
        Color::Rgb(r, g, b) => (r as f32, g as f32, b as f32),
        _ => (255.0, 255.0, 255.0),
    };
    let (ar, ag, ab) = get(a);
    let (br, bg, bb) = get(b);
    let m = |x: f32, y: f32| (x + (y - x) * t).round().clamp(0.0, 255.0) as u8;
    Color::Rgb(m(ar, br), m(ag, bg), m(ab, bb))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the state directory comes from the environment, so tests that
    /// change it have to take turns
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn sandbox(name: &str) -> (std::sync::MutexGuard<'static, ()>, std::path::PathBuf) {
        let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("blur-home-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe { std::env::set_var("BLUR_STATE_DIR", &dir) };
        (guard, dir)
    }

    /// recents only holds real files, so tests need real files
    fn touch(dir: &std::path::Path, name: &str) -> String {
        let p = dir.join(name);
        std::fs::write(&p, "x").unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn recents_are_newest_first_without_duplicates() {
        let (_g, dir) = sandbox("recents");
        let a = touch(&dir, "a.rs");
        let b = touch(&dir, "b.rs");
        record_recent(&a);
        record_recent(&b);
        record_recent(&a);
        assert_eq!(recent_files(), vec![a, b]);
    }

    #[test]
    fn recents_are_capped() {
        let (_g, dir) = sandbox("cap");
        let mut files = Vec::new();
        for i in 0..MAX_RECENTS + 5 {
            files.push(touch(&dir, &format!("f{i}.rs")));
        }
        for f in &files {
            record_recent(f);
        }
        let listed = recent_files();
        assert_eq!(listed.len(), MAX_RECENTS);
        assert_eq!(listed[0], *files.last().unwrap());
    }

    #[test]
    fn rows_are_actions_then_recents_then_quit() {
        let (_g, dir) = sandbox("rows");
        let one = touch(&dir, "one.rs");
        record_recent(&one);
        let mut h = Home::new();
        assert_eq!(h.rows.len(), 4);
        assert_eq!(h.activate(), Some(Choice::Prompt));
        h.cancel();
        h.move_sel(1);
        assert_eq!(h.activate(), Some(Choice::New));
        h.move_sel(1);
        assert_eq!(h.activate(), Some(Choice::Open(one.clone())));
        h.move_sel(1);
        assert_eq!(h.activate(), Some(Choice::Quit));
    }

    #[test]
    fn directories_never_enter_recents() {
        let (_g, dir) = sandbox("dirs");
        let file = dir.join("real.txt");
        std::fs::write(&file, "x").unwrap();
        record_recent(dir.to_str().unwrap());
        assert!(recent_files().is_empty(), "a directory is not openable");
        record_recent(file.to_str().unwrap());
        assert_eq!(recent_files().len(), 1);
    }

    #[test]
    fn deleted_files_are_purged() {
        let (_g, dir) = sandbox("purge");
        let live = dir.join("live.txt");
        std::fs::write(&live, "x").unwrap();
        let gone = dir.join("gone.txt");
        std::fs::write(&gone, "x").unwrap();
        record_recent(gone.to_str().unwrap());
        record_recent(live.to_str().unwrap());
        assert_eq!(recent_files().len(), 2);

        std::fs::remove_file(&gone).unwrap();
        purge_missing();
        let left = recent_files();
        assert_eq!(left.len(), 1, "the deleted entry should be gone");
        assert!(left[0].ends_with("live.txt"));
    }

    #[test]
    fn purge_keeps_everything_when_all_files_exist() {
        let (_g, dir) = sandbox("purge-keep");
        let a = dir.join("a.txt");
        std::fs::write(&a, "x").unwrap();
        record_recent(a.to_str().unwrap());
        purge_missing();
        assert_eq!(recent_files().len(), 1);
    }

    #[test]
    fn selection_wraps_around() {
        let _g = sandbox("wrap");
        let mut h = Home::new();
        h.move_sel(-1);
        assert_eq!(h.sel, h.rows.len() - 1);
        h.move_sel(1);
        assert_eq!(h.sel, 0);
    }
}
