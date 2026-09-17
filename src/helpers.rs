use syntect::parsing::SyntaxSet;



pub struct Visual {
            pub v_x: usize,
            pub v_y: usize,
            pub on : bool,
}

impl Visual {
    pub fn new() -> Self {
        Visual { v_x: 0, v_y: 0, on: false}
    }

}




/////////////////////////////////////////////////////////////////////////////////////////////////////////

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
}

impl Highlighter {
    pub fn new(theme: &opaline::Theme) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntect_theme = opaline::adapters::syntect::to_syntect_theme(theme);
        Highlighter {
            syntax_set,
            syntect_theme,
        }
    }

    pub fn highlight<'a>(&self, tab: &Tab) -> Vec<ratatui::text::Line<'a>> {
        if tab.file_name.is_empty() {
            return tab
                .input_box
                .iter()
                .map(|line| {
                    ratatui::text::Line::from(ratatui::text::Span::styled(
                        line.to_string(),
                        ratatui::style::Style::default().fg(ratatui::style::Color::White),
                    ))
                })
                .collect();
        }
        let syntax = self
            .syntax_set
            .find_syntax_for_file(&tab.file_name)
            .ok()
            .flatten()
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut the_highlighter = syntect::easy::HighlightLines::new(syntax, &self.syntect_theme);
        let mut spans: Vec<ratatui::text::Line> = Vec::new();

        for line in &tab.input_box {
            let range = the_highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            let mut spans_for_line: Vec<ratatui::text::Span> = Vec::new();
            for (style, text) in range {
                let fg = style.foreground;
                let span = ratatui::text::Span::styled(
                    text.trim_end_matches('\n').to_string(),
                    ratatui::style::Style::default()
                        .fg(ratatui::style::Color::Rgb(fg.r, fg.g, fg.b)),
                );
                spans_for_line.push(span);
            }
            spans.push(ratatui::text::Line::from(spans_for_line));
        }
        return spans;
    }
}

///////////////////////////////////////////////////////////////////////////////////

pub struct Tab {
    pub file_name: String,
    pub saved: bool,
    pub input_box: Vec<String>,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub scroll_x: u16,
    pub scroll_y: u16,
    pub undo_stack: Vec<EditRecord>,
    pub redo_stack: Vec<EditRecord>,
}

impl Tab {
    pub fn new() -> Self {
        Self {
            file_name: String::from(""),
            saved: true,
            input_box: vec![String::new()],
            cursor_x: 0,
            cursor_y: 0,
            scroll_y: 0,
            scroll_x: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

pub enum EditRecord {
    InsertChar {
        row: usize,
        col: usize,
        ch: char,
    },
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
        content: String 
    },
    RemoveEmptyLine {
        row: usize,
    },
}

pub fn apply_inverse(record: &EditRecord, input_box: &mut Vec<String>) -> (usize, usize) {
    match record {
        EditRecord::InsertChar { row, col, .. } => {
            input_box[*row].remove(*col);
            (*row, *col)
        }

        EditRecord::DeleteChar { row, col, ch } => {
            input_box[*row].insert(*col, *ch);
            (*row, *col + 1)
        }

        EditRecord::InsertString { row, col, text } => {
            let end = col + text.chars().count();
            input_box[*row].replace_range(*col..end, "");
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
        EditRecord::InsertChar { row, col, ch } => {
            input_box[*row].insert(*col, *ch);
            (*row, *col + ch.len_utf8())
        }

        EditRecord::DeleteChar { row, col, .. } => {
            input_box[*row].remove(*col);
            (*row, *col)
        }

        EditRecord::InsertString { row, col, text } => {
            input_box[*row].insert_str(*col, text);
            (*row, *col + text.len())
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
