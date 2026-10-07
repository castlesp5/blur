//! readme preview pane: side-by-side markdown rendering with live updates,
//! plus terminal image display over the kitty graphics protocol.
//!
//! non-kitty terminals get a clean text placeholder per image. nothing here
//! ever touches the network or leaves the machine.

use std::collections::{HashMap, HashSet};
use std::io::Write;

use ratatui::style::*;
use ratatui::text::*;

/// pixel width assumed per cell column when fitting images.
const PX_PER_COL: u32 = 10;
/// cell rows are roughly twice as tall as wide.
const ROW_PX: u32 = 20;
/// widest an image may render, in cells.
const MAX_IMG_COLS: u16 = 60;
/// tallest an image may render, in rows.
const MAX_IMG_ROWS: u16 = 14;

pub struct PreviewState {
    pub open: bool,
    /// absolute image path -> kitty image id
    pub media: HashMap<String, u32>,
    /// image id -> resized png bytes, kept for re-transmit after deletes
    pub png: HashMap<u32, Vec<u8>>,
    /// image id -> resized pixel dims
    pub dims: HashMap<u32, (u32, u32)>,
    pub next_id: u32,
    /// ids transmitted to the terminal this session
    pub sent: HashSet<u32>,
    /// ids placed on screen last frame, diffed for cleanup
    pub shown: Vec<u32>,
}

impl PreviewState {
    pub fn new() -> Self {
        Self {
            open: false,
            media: HashMap::new(),
            png: HashMap::new(),
            dims: HashMap::new(),
            next_id: 1,
            sent: HashSet::new(),
            shown: Vec::new(),
        }
    }

    pub fn forget(&mut self, id: u32) {
        self.sent.remove(&id);
    }
}

/// one image placement computed during layout, flushed after draw.
pub struct Placement {
    pub id: u32,
    pub x: u16,
    pub y: u16,
    pub cols: u16,
}

/// renderer output: final cursor cell plus kitty placements to flush.
#[derive(Default)]
pub struct FrameOut {
    pub cursor: (u16, u16),
    pub placements: Vec<Placement>,
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
    /// tab bar row
    pub tab_y: u16,
    /// tab pill x-ranges with real tab indices
    pub tab_pills: Vec<(u16, u16, usize)>,
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

/// load, fit, and cache an image. returns its kitty id and reserved rows.
pub fn ensure_media(state: &mut PreviewState, path: &str, cols: u16) -> Option<(u32, u16)> {
    let key = std::path::Path::new(path)
        .canonicalize()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string());
    if let Some(id) = state.media.get(&key).copied() {
        let rows = rows_for(state.dims.get(&id).copied(), cols);
        return Some((id, rows));
    }
    let img = image::open(&key).ok()?;
    let target_w = (cols.min(MAX_IMG_COLS) as u32 * PX_PER_COL).max(16);
    let target_h = MAX_IMG_ROWS as u32 * ROW_PX;
    let fit = img.thumbnail(target_w, target_h);
    let (w, h) = (fit.width().max(1), fit.height().max(1));
    let mut png: Vec<u8> = Vec::new();
    {
        use std::io::Cursor;
        fit.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .ok()?;
    }
    let id = state.next_id;
    state.next_id = state.next_id.wrapping_add(1).max(1);
    state.media.insert(key, id);
    state.png.insert(id, png);
    state.dims.insert(id, (w, h));
    Some((id, rows_for(Some((w, h)), cols)))
}

fn rows_for(dims: Option<(u32, u32)>, cols: u16) -> u16 {
    match dims {
        Some((w, h)) if w > 0 => {
            let disp_w = cols.min(MAX_IMG_COLS) as u32 * PX_PER_COL;
            let scale = disp_w as f64 / w as f64;
            ((h as f64 * scale) / ROW_PX as f64)
                .ceil()
                .max(2.0)
                .min(MAX_IMG_ROWS as f64) as u16
        }
        _ => 4,
    }
}

/// terminals speaking the kitty graphics protocol: kitty itself,
/// wezterm, and ghostty. everything else gets text placeholders.
pub fn kitty_supported() -> bool {
    if std::env::var_os("KITTY_WINDOW_ID").is_some() {
        return true;
    }
    if std::env::var("TERM").unwrap_or_default().contains("kitty") {
        return true;
    }
    // inside tmux or screen the outer terminal may still speak kitty
    // graphics. unknown APC sequences are ignored elsewhere, and a
    // text placeholder sits behind every image, so attempting is safe.
    if std::env::var_os("TMUX").is_some() || std::env::var_os("STY").is_some() {
        return true;
    }
    matches!(
        std::env::var("TERM_PROGRAM").as_deref(),
        Ok("WezTerm") | Ok("ghostty")
    )
}

/// wrap an escape for tmux passthrough when nested in tmux.
fn wrap(cmd: &str) -> String {
    if std::env::var_os("TMUX").is_some() {
        format!("\x1bPtmux;\x1b{}\x1b\\", cmd.replace('\x1b', "\x1b\x1b"))
    } else {
        cmd.to_string()
    }
}

fn emit(out: &mut impl Write, s: &str) {
    let _ = out.write_all(wrap(s).as_bytes());
}

/// transmit cached png bytes, chunked base64, replies silenced with q=2.
/// a lone chunk terminates with m=0, otherwise the terminal waits forever.
pub fn kitty_transmit(out: &mut impl Write, id: u32, png: &[u8]) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    if png.is_empty() {
        return;
    }
    let b64 = STANDARD.encode(png);
    let bytes = b64.as_bytes();
    let total = bytes.chunks(4096).len().max(1);
    for (n, chunk) in bytes.chunks(4096).enumerate() {
        let more = usize::from(n + 1 < total);
        emit(
            out,
            &format!(
                "\x1b_Ga=t,f=100,i={},m={},q=2;{}\x1b\\",
                id,
                more,
                String::from_utf8_lossy(chunk)
            ),
        );
    }
}

/// place a transmitted image at the cursor, width in cells, aspect kept.
pub fn kitty_place(out: &mut impl Write, id: u32, cols: u16) {
    emit(out, &format!("\x1b_Ga=p,i={},c={},q=2\x1b\\", id, cols));
}

/// delete an image and all its placements.
pub fn kitty_delete(out: &mut impl Write, id: u32) {
    emit(out, &format!("\x1b_Ga=d,d=I,i={},q=2\x1b\\", id));
}

/// diff placements against last frame, transmit new ones, place visible
/// ones, delete stale ones, then park the cursor back on the editor.
pub fn flush_media(state: &mut PreviewState, placements: &[Placement], cursor: (u16, u16)) {
    if !kitty_supported() {
        return;
    }
    let out = std::io::stdout();
    let mut lock = out.lock();
    let cur: HashSet<u32> = placements.iter().map(|p| p.id).collect();
    let stale: Vec<u32> = state
        .shown
        .drain(..)
        .filter(|id| !cur.contains(id))
        .collect();
    for id in stale {
        kitty_delete(&mut lock, id);
        state.forget(id);
    }
    // crossterm cursor moves interleave safely after ratatui's draw flush
    for p in placements {
        if !state.sent.contains(&p.id)
            && let Some(png) = state.png.get(&p.id).cloned()
        {
            kitty_transmit(&mut lock, p.id, &png);
            state.sent.insert(p.id);
        }
        let _ = crossterm::execute!(lock, crossterm::cursor::MoveTo(p.x, p.y),);
        kitty_place(&mut lock, p.id, p.cols);
    }
    let _ = crossterm::execute!(lock, crossterm::cursor::MoveTo(cursor.0, cursor.1),);
    let _ = lock.flush();
    state.shown = placements.iter().map(|p| p.id).collect();
}

/// drop every placement, used on close, resize, and exit.
pub fn clear_media(state: &mut PreviewState) {
    if !kitty_supported() {
        state.shown.clear();
        state.sent.clear();
        return;
    }
    let out = std::io::stdout();
    let mut lock = out.lock();
    let stale: Vec<u32> = std::mem::take(&mut state.shown);
    for id in stale {
        kitty_delete(&mut lock, id);
        state.forget(id);
    }
    let _ = lock.flush();
}

// ---------------------------------------------------------------------------
// minimal markdown subset: headings, quotes, lists, code fences, rules,
// bold, italic, strike, inline code, links, images. tables stay raw.
// ---------------------------------------------------------------------------

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
    fn transmit_chunks_reassemble() {
        let png = vec![0u8; 9000];
        let mut out = Vec::new();
        kitty_transmit(&mut out, 7, &png);
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("\x1b_Ga=t,f=100,i=7,m=1,q=2;"));
        assert!(s.contains(",m=0,q=2;"), "last chunk must close with m=0");
        let payload: String = s
            .split(';')
            .skip(1)
            .collect::<Vec<_>>()
            .join("")
            .replace("\x1b\\", "");
        assert_eq!(payload.len() % 4, 0);
    }

    #[test]
    fn place_and_delete_format() {
        let mut out = Vec::new();
        kitty_place(&mut out, 3, 40);
        kitty_delete(&mut out, 3);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("\x1b_Ga=p,i=3,c=40,q=2\x1b\\"));
        assert!(s.contains("\x1b_Ga=d,d=I,i=3,q=2\x1b\\"));
    }

    #[test]
    fn single_chunk_terminates() {
        let png = vec![1u8; 100];
        let mut out = Vec::new();
        kitty_transmit(&mut out, 9, &png);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains(",m=0,q=2;"), "lone chunk must terminate");
        assert!(!s.contains(",m=1,"), "no continuation expected");
    }

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
