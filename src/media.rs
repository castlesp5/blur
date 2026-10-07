//! terminal image support for the preview pane.
//!
//! capability detection is a real query to the terminal rather than a
//! guess from environment variables. that matters: guessing wrong makes
//! the terminal print raw escape sequences all over the screen, while a
//! query either answers or stays quiet.

use std::collections::HashMap;
use std::path::Path;

use ratatui::Frame;
use ratatui::layout::Rect;
pub use ratatui_image::picker::Picker;
use ratatui_image::picker::ProtocolType;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

/// pixels per cell column, and roughly two cells per row
const PX_PER_COL: u32 = 10;
const ROW_PX: u32 = 20;
/// keep images from eating the whole pane
const MAX_COLS: u16 = 60;
const MAX_ROWS: u16 = 16;

pub struct Media {
    picker: Option<Picker>,
    cache: HashMap<String, (StatefulProtocol, (u32, u32))>,
}

/// what the user asked for. `images` is tri state so an explicit
/// `images = true` can force images on inside a multiplexer.
pub struct Images {
    pub requested: Option<bool>,
}

impl Images {
    pub fn from_config(value: Option<bool>) -> Self {
        Self { requested: value }
    }
}

/// build a picker when the terminal is one we can trust to speak the
/// kitty graphics protocol.
///
/// deliberately no capability query. `Picker::from_query_stdio` reads
/// stdin on a thread that outlives its timeout, and when the terminal
/// never answers that thread swallows every keystroke and the editor
/// looks frozen. env signals cost us nothing and cannot hang.
pub fn picker(images: Images) -> Option<Picker> {
    match images.requested {
        // off means off, whatever the environment claims
        Some(false) => None,
        // on means trust the user, they configured passthrough if needed
        Some(true) => Some(kitty_picker()),
        None => {
            if in_multiplexer() {
                // the outer terminal is invisible to us, and asking for
                // passthrough that is not there prints escape garbage
                None
            } else if trusted_terminal() {
                Some(kitty_picker())
            } else {
                None
            }
        }
    }
}

fn kitty_picker() -> Picker {
    let mut p = Picker::halfblocks();
    p.set_protocol_type(ProtocolType::Kitty);
    p
}

fn in_multiplexer() -> bool {
    std::env::var_os("TMUX").is_some() || std::env::var_os("STY").is_some()
}

/// terminals that advertise themselves reliably enough to trust
fn trusted_terminal() -> bool {
    if std::env::var_os("KITTY_WINDOW_ID").is_some() || std::env::var_os("KITTY_PID").is_some() {
        return true;
    }
    if std::env::var("TERM").unwrap_or_default().contains("kitty") {
        return true;
    }
    matches!(
        std::env::var("TERM_PROGRAM").as_deref(),
        Ok("WezTerm") | Ok("ghostty") | Ok("rio")
    )
}

impl Media {
    pub fn new(picker: Option<Picker>) -> Self {
        Self {
            picker,
            cache: HashMap::new(),
        }
    }

    /// true when the terminal answered the capability query
    pub fn supported(&self) -> bool {
        self.picker.is_some()
    }

    /// cached protocol state for a file, decoded and resized on first use
    pub fn protocol(&mut self, path: &str) -> Option<(u32, u32)> {
        let key = path.to_string();
        if let Some((_, dims)) = self.cache.get(&key) {
            return Some(*dims);
        }
        let picker = self.picker.as_ref()?;
        let decoded = image::open(Path::new(path)).ok()?;
        let dims = (decoded.width().max(1), decoded.height().max(1));
        let state = picker.new_resize_protocol(decoded);
        self.cache.insert(key, (state, dims));
        Some(dims)
    }

    /// draw the image into `area`, scaled to fit. caches the encode.
    pub fn draw(&mut self, frame: &mut Frame, path: &str, area: Rect) {
        if area.width == 0 || area.height == 0 || self.picker.is_none() {
            return;
        }
        self.protocol(path);
        let Some((state, _)) = self.cache.get_mut(path) else {
            return;
        };
        let widget = StatefulImage::default().resize(Resize::Fit(None));
        frame.render_stateful_widget(widget, area, state);
    }
}

/// rows an image needs inside a pane `cols` wide, or None when the file
/// is missing or unreadable.
pub fn rows_for(path: &str, cols: u16) -> Option<u16> {
    let decoded = image::open(Path::new(path)).ok()?;
    let w = decoded.width().max(1);
    let h = decoded.height().max(1);
    let disp_w = cols.min(MAX_COLS) as u32 * PX_PER_COL;
    let scale = disp_w as f64 / w as f64;
    let rows = (h as f64 * scale) / ROW_PX as f64;
    Some(rows.ceil().max(2.0).min(MAX_ROWS as f64) as u16)
}

/// same as `rows_for` but for an already decoded image
#[cfg(test)]
pub fn rows_for_dims(dims: (u32, u32), cols: u16) -> u16 {
    let disp_w = cols.min(MAX_COLS) as u32 * PX_PER_COL;
    let scale = disp_w as f64 / dims.0 as f64;
    let rows = (dims.1 as f64 * scale) / ROW_PX as f64;
    rows.ceil().max(2.0).min(MAX_ROWS as f64) as u16
}

/// rect an image occupies inside the preview pane, or None when the row
/// is scrolled out of view. keeps placement honest as the pane scrolls.
pub fn preview_rect(
    pane: Rect,
    row: usize,
    start: usize,
    height: usize,
    view_h: usize,
) -> Option<Rect> {
    if row < start || row >= start + view_h {
        return None;
    }
    let rows = height.min(view_h - (row - start)) as u16;
    if rows == 0 {
        return None;
    }
    Some(Rect::new(
        pane.x,
        pane.y + (row - start) as u16,
        pane.width,
        rows,
    ))
}

/// drop the image cache, used when a document is saved under a new name
pub fn clear(media: &mut Media) {
    media.cache.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_files_have_no_rows() {
        assert_eq!(rows_for("/definitely/not/here.png", 40), None);
    }

    #[test]
    fn row_math_scales_with_aspect_ratio() {
        // a wide image needs fewer rows than a tall one of equal width
        let wide = rows_for_dims((400, 100), 40);
        let tall = rows_for_dims((100, 400), 40);
        assert!(wide < tall, "wide={wide} tall={tall}");
        assert!(wide >= 2);
    }

    #[test]
    fn rows_are_clamped_to_the_pane() {
        for cols in [8u16, 40, 200] {
            let rows = rows_for_dims((10, 5000), cols);
            assert!((2..=MAX_ROWS).contains(&rows), "cols={cols} rows={rows}");
        }
    }

    #[test]
    fn rects_follow_the_scrolled_window() {
        let pane = Rect::new(40, 5, 30, 10);
        // first visible row sits at the top of the pane
        let r = preview_rect(pane, 10, 10, 4, 10).unwrap();
        assert_eq!((r.x, r.y, r.width, r.height), (40, 5, 30, 4));
        // the next image is one row further down
        let r = preview_rect(pane, 14, 10, 4, 10).unwrap();
        assert_eq!(r.y, 9);
        // scrolled out of view means no rect at all
        assert!(preview_rect(pane, 9, 10, 4, 10).is_none());
        assert!(preview_rect(pane, 20, 10, 4, 10).is_none());
        // an image taller than the pane is cut at the bottom
        let r = preview_rect(pane, 18, 10, 30, 10).unwrap();
        assert_eq!(r.y, 13);
        assert_eq!(r.height, 2);
    }

    #[test]
    fn media_without_a_picker_never_reaches_for_an_image() {
        let mut media = Media::new(None);
        assert!(!media.supported());
        assert_eq!(media.protocol("/tmp/whatever.png"), None);
    }

    #[test]
    fn images_off_never_builds_a_picker() {
        assert!(picker(Images::from_config(Some(false))).is_none());
    }

    #[test]
    fn unknown_terminals_get_placeholders_not_a_picker() {
        // the test process runs with no kitty or wezterm markers, so the
        // conservative default has to be off
        if !trusted_terminal() {
            assert!(picker(Images::from_config(None)).is_none());
        }
    }

    #[test]
    fn an_explicit_opt_in_always_builds_a_picker() {
        assert!(picker(Images::from_config(Some(true))).is_some());
    }
}
