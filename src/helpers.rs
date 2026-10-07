use crate::lsp::{self, Diag};
use opaline::Theme;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use syntect::parsing::SyntaxSet;

pub struct Visual {
    pub v_x: usize,
    pub v_y: usize,
    pub on: bool,
}

impl Visual {
    pub fn new() -> Self {
        Visual {
            v_x: 0,
            v_y: 0,
            on: false,
        }
    }
}

fn wcag_contrast(l1: f64, l2: f64) -> f64 {
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

pub fn fg_color(bg: opaline::OpalineColor) -> ratatui::style::Color {
    let to_linear = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * to_linear(bg.r) + 0.7152 * to_linear(bg.g) + 0.0722 * to_linear(bg.b);

    let contrast_black = wcag_contrast(luminance, 0.0);
    let contrast_white = wcag_contrast(luminance, 1.0);

    if contrast_black >= contrast_white {
        ratatui::style::Color::Black
    } else {
        ratatui::style::Color::White
    }
}

pub struct Highlighter {
    syntax_set: syntect::parsing::SyntaxSet,
    syntect_theme: syntect::highlighting::Theme,
    diag_colors: [Color; 4],
}

impl Highlighter {
    pub fn new(theme: &opaline::Theme) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntect_theme = opaline::adapters::syntect::to_syntect_theme(theme);
        Highlighter {
            syntax_set,
            syntect_theme,
            diag_colors: diag_colors(theme),
        }
    }
    fn syntax_lines(&self, tab: &Tab) -> Vec<Line<'static>> {
        if tab.file_name.is_empty() {
            return tab
                .input_box
                .iter()
                .map(|l| Line::from(Span::styled(l.clone(), Style::default().fg(Color::White))))
                .collect();
        }
        let syntax = self
            .syntax_set
            .find_syntax_for_file(&tab.file_name)
            .ok()
            .flatten()
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        let mut h = syntect::easy::HighlightLines::new(syntax, &self.syntect_theme);
        tab.input_box
            .iter()
            .map(|line| {
                let spans: Vec<Span<'static>> = h
                    .highlight_line(line, &self.syntax_set)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(st, text)| {
                        Span::styled(
                            text.trim_end_matches('\n').to_string(),
                            Style::default().fg(Color::Rgb(
                                st.foreground.r,
                                st.foreground.g,
                                st.foreground.b,
                            )),
                        )
                    })
                    .collect();
                Line::from(spans)
            })
            .collect()
    }

    pub fn highlight(&self, tab: &mut Tab, diags: &[Diag], height: usize) -> Vec<Line<'static>> {
        if tab.highlight_cache.is_none() {
            tab.highlight_cache = Some(self.syntax_lines(tab));
        }
        let base = tab.highlight_cache.as_ref().unwrap();
        base.iter()
            .enumerate()
            .skip(tab.scroll_y as usize)
            .take(height)
            .map(|(i, line)| {
                let text = tab.input_box.get(i).map_or("", |s| s.as_str());
                let marks = marks_for_row(i as u32, text, diags);
                Line::from(overlay(line.spans.clone(), &marks, &self.diag_colors))
            })
            .collect()
    }
}

fn diag_colors(theme: &Theme) -> [ratatui::style::Color; 4] {
    [
        theme.color("error").into(),
        theme.color("warning").into(),
        theme.color("info").into(),
        theme.color("text.muted").into(),
    ]
}

fn underline(sev: u8, colors: &[ratatui::style::Color; 4]) -> Style {
    let c = colors[(sev.clamp(1, 4) - 1) as usize];
    Style::default()
        .fg(c)
        .add_modifier(Modifier::UNDERLINED)
        .underline_color(c)
}

// aio lsp shtuff

pub fn marks_for_row(row: u32, line: &str, diags: &[Diag]) -> Vec<(usize, usize, u8)> {
    let mut marks = Vec::new();
    for d in diags {
        if row < d.start.0 || row > d.end.0 {
            continue;
        }
        let s = if row == d.start.0 {
            lsp::utf16_to_byte(line, d.start.1)
        } else {
            0
        };
        let mut e = if row == d.end.0 {
            lsp::utf16_to_byte(line, d.end.1)
        } else {
            line.len()
        };
        if e <= s {
            e = line[s..].chars().next().map_or(s, |c| s + c.len_utf8());
        }
        if e > s {
            marks.push((s, e, d.severity));
        }
    }
    marks
}

pub fn overlay(
    spans: Vec<Span<'static>>,
    marks: &[(usize, usize, u8)],
    colors: &[Color; 4],
) -> Vec<Span<'static>> {
    if marks.is_empty() {
        return spans;
    }
    let piece = |text: String, base: Style, sev: Option<u8>| match sev {
        Some(s) => Span::styled(text, base.patch(underline(s, colors))),
        None => Span::styled(text, base),
    };
    let mut out = Vec::new();
    let mut pos = 0usize;
    for span in spans {
        let mut buf = String::new();
        let mut cur: Option<u8> = None;
        for ch in span.content.chars() {
            let sev = marks
                .iter()
                .filter(|m| pos >= m.0 && pos < m.1)
                .map(|m| m.2)
                .min();
            if sev != cur && !buf.is_empty() {
                out.push(piece(std::mem::take(&mut buf), span.style, cur));
            }
            cur = sev;
            buf.push(ch);
            pos += ch.len_utf8();
        }
        if !buf.is_empty() {
            out.push(piece(buf, span.style, cur));
        }
    }
    out
}

///////////////////////////////////////////////////////////////////////////////////

pub struct Tab {
    pub file_name: String,
    pub saved: bool,
    pub highlight_cache: Option<Vec<ratatui::text::Line<'static>>>,
    pub input_box: Vec<String>,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub scroll_x: u16,
    pub scroll_y: u16,
    pub undo_stack: Vec<EditRecord>,
    pub redo_stack: Vec<EditRecord>,
    pub lsp_dirty: bool,
    pub lsp_file: String,
    pub lsp_sent: String,
}

impl Tab {
    pub fn new() -> Self {
        Self {
            file_name: String::from(""),
            saved: false,
            highlight_cache: None,
            input_box: vec![String::new()],
            cursor_x: 0,
            cursor_y: 0,
            scroll_y: 0,
            scroll_x: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            lsp_dirty: false,
            lsp_file: String::new(),
            lsp_sent: String::new(),
        }
    }

    pub fn unsave(&mut self) {
        self.saved = false;
        self.highlight_cache = None;
        self.lsp_dirty = true;
    }
}

pub enum EditRecord {
    DeleteChar {
        row: usize,
        col: usize,
        ch: char,
    },
    InsertString {
        row: usize,
        col: usize,
        text: String,
    },
    RemoveString {
        row: usize,
        col: usize,
        text: String,
    },
    SplitLine {
        row: usize,
        col: usize,
    },
    MergeLine {
        row: usize,
        prev_len: usize,
    },
    InsertLine {
        row: usize,
    },
    RemoveLine {
        row: usize,
        content: String,
    },
    RemoveEmptyLine {
        row: usize,
    },
}

pub fn apply_inverse(record: &EditRecord, input_box: &mut Vec<String>) -> (usize, usize) {
    match record {
        EditRecord::DeleteChar { row, col, ch } => {
            input_box[*row].insert(*col, *ch);
            (*row, *col + ch.len_utf8())
        }

        EditRecord::InsertString { row, col, text } => {
            let end = col + text.len();
            input_box[*row].replace_range(*col..end, "");
            (*row, *col)
        }

        EditRecord::RemoveString { row, col, text } => {
            input_box[*row].insert_str(*col, text);
            (*row, *col)
        }

        EditRecord::SplitLine { row, col } => {
            let next_line = input_box.remove(*row + 1);
            input_box[*row].push_str(&next_line);
            (*row, *col)
        }

        EditRecord::MergeLine { row, prev_len } => {
            let combined = input_box[*row - 1].split_off(*prev_len);
            input_box.insert(*row, combined);
            (*row, 0)
        }

        EditRecord::InsertLine { row } => {
            input_box.remove(*row);
            (*row, 0)
        }

        EditRecord::RemoveLine { row, content } => {
            input_box.insert(*row, content.clone());
            (*row, 0)
        }
        EditRecord::RemoveEmptyLine { row } => {
            input_box.insert(*row, String::new());
            (*row, 0)
        }
    }
}

pub fn apply_forward(record: &EditRecord, input_box: &mut Vec<String>) -> (usize, usize) {
    match record {
        EditRecord::DeleteChar { row, col, .. } => {
            input_box[*row].remove(*col);
            (*row, *col)
        }

        EditRecord::InsertString { row, col, text } => {
            input_box[*row].insert_str(*col, text);
            (*row, *col + text.len())
        }

        EditRecord::RemoveString { row, col, text } => {
            let end = col + text.len();
            input_box[*row].replace_range(*col..end, "");
            (*row, *col)
        }

        EditRecord::SplitLine { row, col } => {
            let rest = input_box[*row].split_off(*col);
            input_box.insert(*row + 1, rest);
            (*row + 1, 0)
        }

        EditRecord::MergeLine { row, prev_len } => {
            let combined = input_box.remove(*row + 1);
            input_box[*row].push_str(&combined);
            (*row, *prev_len)
        }

        EditRecord::InsertLine { row } => {
            input_box.insert(*row, String::new());
            (*row, 0)
        }
        EditRecord::RemoveLine { row, .. } => {
            input_box.remove(*row);
            (*row, 0)
        }

        EditRecord::RemoveEmptyLine { row } => {
            input_box.remove(*row);
            (*row, 0)
        }
    }
}

#[allow(dead_code)]
pub fn log(msg: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("blur-log.txt")
    {
        writeln!(file, "{}", msg).ok();
    }
}
