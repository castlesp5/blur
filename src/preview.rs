//! readme preview pane: side-by-side markdown rendering with live updates,
//! plus terminal image display over the kitty graphics protocol.
//!
//! non-kitty terminals get a clean text placeholder per image. nothing here
//! ever touches the network or leaves the machine.

use ratatui::style::*;
use ratatui::text::*;

/// the parsed document, kept until the buffer actually changes
struct DocCache {
    name: String,
    revision: u64,
    rows: Vec<DocRow>,
}

pub struct PreviewState {
    pub open: bool,
    doc: Option<DocCache>,
}

impl PreviewState {
    pub fn new() -> Self {
        Self {
            open: false,
            doc: None,
        }
    }

    /// parsed rows for the current buffer. re-parsing a document on
    /// every frame was the second real cost in the preview, and it is
    /// pure waste when nothing changed since the last frame.
    pub fn doc_rows<'a>(
        state: &'a mut PreviewState,
        name: &str,
        revision: u64,
        lines: &[String],
        colors: &MdColors,
    ) -> &'a [DocRow] {
        let fresh = state
            .doc
            .as_ref()
            .is_some_and(|c| c.revision == revision && c.name == name);
        if !fresh {
            state.doc = Some(DocCache {
                name: name.to_string(),
                revision,
                rows: parse_markdown(lines, colors),
            });
        }
        state
            .doc
            .as_ref()
            .map(|c| c.rows.as_slice())
            .unwrap_or_default()
    }
}

/// clickable layout snapshot from the last frame. mouse events map
/// cells back through these rects, so every control is clickable.
#[derive(Default)]
pub struct UiLayout {
    /// editor code area inside its box
    pub edit_inner: ratatui::layout::Rect,
    /// preview code area inside its box, when split
    pub prev_inner: Option<ratatui::layout::Rect>,
    /// gutter cells before code in that area
    pub gutter_w: u16,
    /// usable code width, excluding gutter and scrollbar
    pub text_w: usize,
    /// tab bar row
    pub tab_y: u16,
    /// tab pill x-ranges with real tab indices
    pub tab_pills: Vec<(u16, u16, usize)>,
    /// close button x-ranges per tab
    pub tab_close: Vec<(u16, u16, usize)>,
    /// last pointer position, drives hover states
    pub mouse: Option<(u16, u16)>,
    /// theme picker popup: area, first row, row count
    pub picker: Option<(ratatui::layout::Rect, usize, usize)>,
    /// confirm modal area, left half confirms
    pub modal: Option<ratatui::layout::Rect>,
}

/// convert a click cell into buffer coordinates, char-boundary safe.
#[allow(clippy::too_many_arguments)]
pub fn cell_to_buffer(
    lines: &[String],
    inner_x: u16,
    gutter_w: u16,
    scroll_x: u16,
    scroll_y: u16,
    cx: u16,
    cy: u16,
    inner_y: u16,
) -> (i32, i32) {
    let row = (cy.saturating_sub(inner_y) as usize + scroll_y as usize)
        .min(lines.len().saturating_sub(1));
    let want = cx.saturating_sub(inner_x).saturating_sub(gutter_w) as usize + scroll_x as usize;
    let line = &lines[row];
    let mut width = 0usize;
    let mut byte = 0usize;
    for (idx, ch) in line.char_indices() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        if width + w > want {
            break;
        }
        width += w;
        byte = idx + ch.len_utf8();
    }
    (row as i32, byte as i32)
}

#[derive(Clone)]
pub enum DocRow {
    Text(Line<'static>),
    /// image reference, row reservation computed at layout time
    Media {
        alt: String,
        path: String,
    },
}

pub struct MdColors {
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
    pub accent: Color,
    pub code: Color,
    pub quote: Color,
}

fn push_plain(spans: &mut Vec<Span<'static>>, buf: &mut String, c: &MdColors) {
    if !buf.is_empty() {
        spans.push(Span::styled(
            std::mem::take(buf),
            Style::default().fg(c.text),
        ));
    }
}

pub fn inline(text: &str, c: &MdColors) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut buf = String::new();
    macro_rules! plain {
        () => {
            push_plain(&mut spans, &mut buf, c)
        };
    }
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let take_until = |pat: &str, from: usize| -> Option<usize> {
        let pat: Vec<char> = pat.chars().collect();
        let mut k = from;
        while k + pat.len() <= chars.len() {
            if chars[k..k + pat.len()] == pat[..] {
                return Some(k);
            }
            // skip escaped
            if chars[k] == '\\' {
                k += 1;
            }
            k += 1;
        }
        None
    };
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        if rest.starts_with("**") {
            if let Some(end) = take_until("**", i + 2) {
                plain!();
                spans.push(Span::styled(
                    chars[i + 2..end].iter().collect::<String>(),
                    Style::default().fg(c.text).add_modifier(Modifier::BOLD),
                ));
                i = end + 2;
                continue;
            }
        } else if rest.starts_with("~~") {
            if let Some(end) = take_until("~~", i + 2) {
                plain!();
                spans.push(Span::styled(
                    chars[i + 2..end].iter().collect::<String>(),
                    Style::default()
                        .fg(c.dim)
                        .add_modifier(Modifier::CROSSED_OUT),
                ));
                i = end + 2;
                continue;
            }
        } else if rest.starts_with('`') {
            if let Some(end) = take_until("`", i + 1) {
                plain!();
                spans.push(Span::styled(
                    chars[i + 1..end].iter().collect::<String>(),
                    Style::default().fg(c.code),
                ));
                i = end + 1;
                continue;
            }
        } else if rest.starts_with('!') && rest[1..].starts_with('[') {
            // image inside prose: show alt only, block parser owns placement
            if let Some(mid) = take_until("]", i + 2) {
                plain!();
                spans.push(Span::styled(
                    chars[i + 2..mid].iter().collect::<String>(),
                    Style::default().fg(c.accent).add_modifier(Modifier::BOLD),
                ));
                // skip (src...) target
                let after: String = chars[mid..].iter().collect();
                if after.starts_with("](")
                    && let Some(end) = take_until(")", mid + 2)
                {
                    i = end + 1;
                    continue;
                }
                i = mid + 1;
                continue;
            }
        } else if rest.starts_with('[') {
            if let Some(mid) = take_until("]", i + 1) {
                let after: String = chars[mid..].iter().collect();
                if after.starts_with("](")
                    && let Some(end) = take_until(")", mid + 2)
                {
                    plain!();
                    spans.push(Span::styled(
                        chars[i + 1..mid].iter().collect::<String>(),
                        Style::default()
                            .fg(c.accent)
                            .add_modifier(Modifier::UNDERLINED),
                    ));
                    let url: String = chars[mid + 2..end].iter().collect();
                    spans.push(Span::styled(
                        format!(" ({})", url),
                        Style::default().fg(c.dim),
                    ));
                    i = end + 1;
                    continue;
                }
            }
        } else if rest.starts_with('*') || rest.starts_with('_') {
            let mark = if rest.starts_with('*') { "*" } else { "_" };
            if let Some(end) = take_until(mark, i + 1)
                && end > i + 1
            {
                plain!();
                spans.push(Span::styled(
                    chars[i + 1..end].iter().collect::<String>(),
                    Style::default().fg(c.text).add_modifier(Modifier::ITALIC),
                ));
                i = end + 1;
                continue;
            }
        } else if rest.starts_with('\\') && chars.len() > i + 1 {
            buf.push(chars[i + 1]);
            i += 2;
            continue;
        }
        buf.push(chars[i]);
        i += 1;
    }
    plain!();
    spans
}

/// split a line into an image ref when it is exactly `![alt](src)`.
/// remote or inline sources never resolve to local media.
fn remote_src(src: &str) -> bool {
    src.is_empty()
        || src.starts_with("http://")
        || src.starts_with("https://")
        || src.starts_with("data:")
}

/// pull a quoted html attribute value out of a tag.
fn html_attr(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let key = format!("{}={}", name, quote);
        if let Some(at) = tag.find(&key) {
            let rest = &tag[at + key.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

/// image ref for a `![alt](src)` line or an `<img src="...">` tag.
/// alt falls back to the file name.
fn line_image(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    if t.starts_with("![") {
        let mid = t.find("](")?;
        let end = t.rfind(')')?;
        if mid + 2 > end {
            return None;
        }
        let alt = t[2..mid].to_string();
        let mut src = t[mid + 2..end].to_string();
        // drop optional "title"
        if let Some(sp) = src.find(" \"") {
            src.truncate(sp);
        }
        src = src.trim().trim_matches(&['<', '>'][..]).to_string();
        if remote_src(&src) {
            return None;
        }
        return Some((alt, src));
    }
    if let Some(at) = t.find("<img") {
        let tag = &t[at..];
        let end = tag.find('>')?;
        let tag = &tag[..end];
        let src = html_attr(tag, "src")?;
        if remote_src(&src) {
            return None;
        }
        let alt = html_attr(tag, "alt")
            .unwrap_or_else(|| src.rsplit('/').next().unwrap_or(&src).to_string());
        return Some((alt, src));
    }
    None
}

pub fn parse_markdown(lines: &[String], c: &MdColors) -> Vec<DocRow> {
    let mut rows = Vec::new();
    let mut in_fence = false;
    for line in lines {
        let t = line.trim_start();
        if t.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            rows.push(DocRow::Text(Line::from(vec![Span::styled(
                format!("  {}", line),
                Style::default().fg(c.code),
            )])));
            continue;
        }
        if let Some((alt, src)) = line_image(line) {
            rows.push(DocRow::Media { alt, path: src });
            continue;
        }
        if t.is_empty() {
            rows.push(DocRow::Text(Line::from("")));
            continue;
        }
        // headings: deeper level, quieter style. h1 gets an underline.
        let mut headed = false;
        for (depth, mark) in ["###### ", "##### ", "#### ", "### ", "## ", "# "]
            .iter()
            .enumerate()
        {
            if let Some(body) = t.strip_prefix(mark) {
                let style = if depth == 5 {
                    Style::default()
                        .fg(c.accent)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().fg(c.accent).add_modifier(Modifier::BOLD)
                };
                rows.push(DocRow::Text(Line::from(vec![Span::styled(
                    body.to_string(),
                    style,
                )])));
                headed = true;
                break;
            }
        }
        if headed {
            continue;
        }
        if t == "---" || t == "***" || t == "___" {
            rows.push(DocRow::Text(Line::from(vec![Span::styled(
                "─".repeat(24),
                Style::default().fg(c.faint),
            )])));
            continue;
        }
        if let Some(q) = t.strip_prefix("> ") {
            let mut spans = vec![Span::styled("│ ", Style::default().fg(c.quote))];
            spans.extend(inline(q, c));
            rows.push(DocRow::Text(Line::from(spans)));
            continue;
        }
        // lists: indent 2 per level, bullet per marker
        let stripped = line.trim_start();
        let indent = line.len() - stripped.len();
        let level = indent / 2;
        let pad = "  ".repeat(level);
        if let Some(rest) = stripped
            .strip_prefix("- ")
            .or_else(|| stripped.strip_prefix("* "))
            .or_else(|| stripped.strip_prefix("+ "))
        {
            let mut spans = vec![
                Span::raw(pad),
                Span::styled("• ", Style::default().fg(c.accent)),
            ];
            spans.extend(inline(rest, c));
            rows.push(DocRow::Text(Line::from(spans)));
            continue;
        }
        // ordered list: digits + dot
        let mut digits = 0;
        for ch in stripped.chars() {
            if ch.is_ascii_digit() {
                digits += 1;
            } else {
                break;
            }
        }
        if digits > 0 && stripped[digits..].starts_with(". ") {
            let mut spans = vec![
                Span::raw(pad),
                Span::styled(
                    format!("{}. ", &stripped[..digits]),
                    Style::default().fg(c.accent),
                ),
            ];
            spans.extend(inline(&stripped[digits + 2..], c));
            rows.push(DocRow::Text(Line::from(spans)));
            continue;
        }
        rows.push(DocRow::Text(Line::from(inline(line, c))));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_line_parsing() {
        assert_eq!(
            line_image("![alt](pic.png)").unwrap(),
            ("alt".to_string(), "pic.png".to_string())
        );
        assert_eq!(
            line_image("  ![a b](d/e.jpg \"t\")").unwrap(),
            ("a b".to_string(), "d/e.jpg".to_string())
        );
        assert!(line_image("![a](https://x/y.png)").is_none());
        assert!(line_image("not an image").is_none());
        assert_eq!(
            line_image("<img src=\"pic/photo.jpg\" alt=\"hi\" width=\"100\">").unwrap(),
            ("hi".to_string(), "pic/photo.jpg".to_string())
        );
        assert_eq!(
            line_image("<img src='a.png'>").unwrap(),
            ("a.png".to_string(), "a.png".to_string())
        );
        assert!(line_image("<img src=\"https://x/y.png\">").is_none());
    }

    #[test]
    fn inline_marks() {
        let c = MdColors {
            text: Color::White,
            dim: Color::Gray,
            faint: Color::DarkGray,
            accent: Color::Blue,
            code: Color::Green,
            quote: Color::Gray,
        };
        let spans = inline("a **b** and `c`", &c);
        assert!(spans.iter().any(|s| s.content == "b"));
        assert!(spans.iter().any(|s| s.content == "c"));
    }
}
