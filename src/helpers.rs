use syntect::parsing::SyntaxSet;

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
