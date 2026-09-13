use syntect::parsing::SyntaxSet;

pub struct Highlighter {
    syntax_set: syntect::parsing::SyntaxSet,
    theme: syntect::highlighting::Theme,
}

impl Highlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = syntect::highlighting::ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-mocha.dark"].clone();
        Highlighter { syntax_set, theme }
    }

    pub fn highlight<'a>(&self, tab: &Tab) -> Vec<ratatui::text::Line<'a>> {
        if tab.file_name.is_empty() {
            return tab
                .input_box
                .split('\n')
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

        let mut the_highlighter = syntect::easy::HighlightLines::new(syntax, &self.theme);
        let mut spans: Vec<ratatui::text::Line> = Vec::new();

        for line in syntect::util::LinesWithEndings::from(&tab.input_box) {
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
    pub input_box: String,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub gcursor: i32,
    pub scroll_x: u16,
    pub scroll_y: u16,
}

impl Tab {
    pub fn new() -> Self {
        Self {
            file_name: String::from(""),
            input_box: String::new(),
            cursor_x: 0,
            cursor_y: 0,
            gcursor: 0,
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
