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

/// resolve a hue across every opaline theme. only the catppuccin
/// family defines raw names like "blue" or "mauve", so each hue falls
/// back through shared semantic tokens. never returns FALLBACK.
pub fn hue(theme: &opaline::Theme, hue: &str) -> opaline::OpalineColor {
    if let Some(c) = theme.try_color(hue) {
        return c;
    }
    // Map missing theme keys strictly to the theme's native semantic tokens
    let mapped = match hue {
        "blue" | "sapphire" | "sky" | "teal" => "accent.secondary",
        "green" => "success",
        "mauve" | "pink" | "lavender" => "accent.primary",
        "peach" | "yellow" => "warning",
        "red" => "error",
        _ => "accent.primary",
    };
    if let Some(c) = theme.try_color(mapped) {
        return c;
    }
    theme
        .try_color("accent.primary")
        .or_else(|| theme.try_color("text.primary"))
        .unwrap_or(opaline::OpalineColor {
            r: 203,
            g: 166,
            b: 247,
        })
}

/// resolve a structural token with a safe fallback chain.
pub fn tok(theme: &opaline::Theme, key: &str) -> opaline::OpalineColor {
    let fallbacks: &[&str] = match key {
        "border.unfocused" => &["text.dim"],
        "accent.deep" => &["accent.primary"],
        _ => &[],
    };
    theme
        .try_color(key)
        .or_else(|| fallbacks.iter().find_map(|k| theme.try_color(k)))
        .unwrap_or_else(|| theme.color("text.primary"))
}

/// blend a color toward the theme background. ratatui and terminals
/// have no alpha channel, so translucent colors are pre-multiplied
/// here instead of being silently ignored.
pub fn blend(
    fg: opaline::OpalineColor,
    bg: opaline::OpalineColor,
    alpha: f32,
) -> opaline::OpalineColor {
    let a = alpha.clamp(0.0, 1.0);
    let mix = |f: u8, b: u8| {
        ((f as f32 * a) + (b as f32 * (1.0 - a)))
            .round()
            .clamp(0.0, 255.0) as u8
    };
    opaline::OpalineColor {
        r: mix(fg.r, bg.r),
        g: mix(fg.g, bg.g),
        b: mix(fg.b, bg.b),
    }
}

/// translucent color over the theme background, ready for ratatui.
pub fn fade(theme: &opaline::Theme, c: opaline::OpalineColor, alpha: f32) -> ratatui::style::Color {
    blend(c, theme.color("bg.base"), alpha).into()
}

/// same, for a color already converted to ratatui.
pub fn fade_rgb(
    theme: &opaline::Theme,
    c: ratatui::style::Color,
    alpha: f32,
) -> ratatui::style::Color {
    match c {
        ratatui::style::Color::Rgb(r, g, b) => blend(
            opaline::OpalineColor { r, g, b },
            theme.color("bg.base"),
            alpha,
        )
        .into(),
        other => other,
    }
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
    md: crate::preview::MdColors,
}

impl Highlighter {
    pub fn new(theme: &opaline::Theme) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntect_theme = opaline::adapters::syntect::to_syntect_theme(theme);
        Highlighter {
            syntax_set,
            syntect_theme,
            md: md_colors(theme),
        }
    }

    /// cheap theme swap for live picker preview: syntaxes stay loaded,
    /// only the syntect theme rebuilds. callers must clear tab caches.
    pub fn set_theme(&mut self, theme: &opaline::Theme) {
        self.syntect_theme = opaline::adapters::syntect::to_syntect_theme(theme);
        self.md = md_colors(theme);
    }

    /// auto detect language: file path first, then shebang / first line,
    /// then content sniff, plain text last. works for untitled buffers too.
    fn detect_syntax(&self, tab: &Tab) -> &syntect::parsing::SyntaxReference {
        if !tab.file_name.is_empty()
            && let Ok(Some(s)) = self.syntax_set.find_syntax_for_file(&tab.file_name)
        {
            return s;
        }
        let first = tab.input_box.first().map(String::as_str).unwrap_or("");
        if !first.is_empty() {
            if let Some(s) = self.syntax_set.find_syntax_by_first_line(first) {
                return s;
            }
            // shebang without syntect first-line match, map common interpreters
            let low = first.to_lowercase();
            let ext = if low.contains("python") {
                Some("py")
            } else if low.contains("node") || low.contains("deno") || low.contains("bun") {
                Some("js")
            } else if low.contains("bash") || low.contains("sh") {
                Some("sh")
            } else if low.contains("ruby") {
                Some("rb")
            } else if low.contains("perl") {
                Some("pl")
            } else if low.contains("lua") {
                Some("lua")
            } else {
                None
            };
            if let Some(e) = ext
                && let Some(s) = self.syntax_set.find_syntax_by_extension(e)
            {
                return s;
            }
        }
        // content sniff for untitled buffers: braces + semicolons smell like c,
        // def/end like ruby, fn/let like rust
        if tab.file_name.is_empty() {
            let sample: String = tab
                .input_box
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            let low = sample.to_lowercase();
            for (needle, ext) in [
                ("fn ", "rs"),
                ("let mut", "rs"),
                ("import ", "py"),
                ("def ", "py"),
                ("console.log", "js"),
                ("package main", "go"),
                ("#include", "c"),
                ("public static void", "java"),
            ] {
                if low.contains(needle)
                    && let Some(s) = self.syntax_set.find_syntax_by_extension(ext)
                {
                    return s;
                }
            }
        }
        self.syntax_set.find_syntax_plain_text()
    }
}

fn md_colors(theme: &opaline::Theme) -> crate::preview::MdColors {
    use ratatui::style::Color;
    let c = |v: opaline::OpalineColor| Color::Rgb(v.r, v.g, v.b);
    crate::preview::MdColors {
        text: c(tok(theme, "text.primary")),
        dim: c(tok(theme, "text.dim")),
        faint: c(tok(theme, "border.unfocused")),
        accent: c(tok(theme, "accent.deep")),
        code: c(hue(theme, "green")),
        quote: c(tok(theme, "text.dim")),
    }
}

impl Highlighter {
    pub fn highlight<'a>(&self, tab: &mut Tab) -> Vec<ratatui::text::Line<'a>> {
        if let Some(cached) = &tab.highlight_cache {
            return cached.clone();
        }
        let syntax = self.detect_syntax(tab);
        // syntect themes carry no markup scopes, so markdown gets a
        // dedicated pass: real colors in the editor, not flat text.
        if syntax.name == "Markdown" {
            let mut out = Vec::new();
            let mut fence = false;
            for line in &tab.input_box {
                if line.trim_start().starts_with("```") {
                    fence = !fence;
                    out.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            line.to_string(),
                            ratatui::style::Style::default().fg(self.md.dim),
                        ),
                    ]));
                    continue;
                }
                if fence {
                    out.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            line.to_string(),
                            ratatui::style::Style::default().fg(self.md.code),
                        ),
                    ]));
                    continue;
                }
                let t = line.trim_start();
                let hashes = t.chars().take_while(|c| *c == '#').count();
                if hashes > 0 && t[hashes..].starts_with(' ') {
                    let mut spans = vec![ratatui::text::Span::styled(
                        " ".repeat(line.len() - t.len()) + &"#".repeat(hashes) + " ",
                        ratatui::style::Style::default().fg(self.md.dim),
                    )];
                    spans.extend(crate::preview::inline(
                        t[hashes + 1..].trim_start(),
                        &self.md,
                    ));
                    // headings read bold without any background wash
                    for s in spans.iter_mut().skip(1) {
                        s.style = s.style.add_modifier(ratatui::style::Modifier::BOLD);
                    }
                    out.push(ratatui::text::Line::from(spans));
                    continue;
                }
                out.push(ratatui::text::Line::from(crate::preview::inline(
                    line, &self.md,
                )));
            }
            tab.highlight_cache = Some(out.clone());
            return out;
        }

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
        tab.highlight_cache = Some(spans.clone());
        spans
    }
}

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
        }
    }

    pub fn unsave(&mut self) {
        self.saved = false;
        self.highlight_cache = None;
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
    /// enter between `{` and `}` (or `[]`, `()`): one keypress makes
    /// three lines. single undo unit via stored tail.
    BraceSplit {
        row: usize,
        col: usize,
        indent: String,
        base: String,
        tail: String,
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

        EditRecord::BraceSplit { row, col, tail, .. } => {
            input_box.remove(*row + 2);
            input_box.remove(*row + 1);
            input_box[*row].push_str(tail);
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

        EditRecord::BraceSplit {
            row,
            col,
            indent,
            base,
            tail,
        } => {
            let head_len = input_box[*row].len().saturating_sub(tail.len());
            let col = (*col).min(head_len);
            input_box[*row].truncate(col);
            input_box.insert(*row + 1, indent.clone());
            input_box.insert(*row + 2, format!("{base}{tail}"));
            (*row + 1, indent.len())
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

/// debug logger for a fullscreen app where stdout is unusable.
/// writes to blur-log.txt in the working directory.
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
