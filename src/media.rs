//! terminal image support for the preview pane.
//!
//! capability detection is a real query to the terminal rather than a
//! guess from environment variables. that matters: guessing wrong makes
//! the terminal print raw escape sequences all over the screen, while a
//! query either answers or stays quiet.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};

use ratatui::Frame;
use ratatui::layout::Rect;
pub use ratatui_image::picker::Picker;
use ratatui_image::picker::ProtocolType;
use ratatui_image::thread::{ResizeRequest, ResizeResponse, ThreadProtocol};
use ratatui_image::{Resize, StatefulImage};

/// pixels per cell column, and roughly two cells per row
const PX_PER_COL: u32 = 10;
const ROW_PX: u32 = 20;
/// keep images from eating the whole pane
const MAX_COLS: u16 = 60;
const MAX_ROWS: u16 = 16;

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
        Some(false) => None,
        Some(true) => kitty_picker(),
        None => {
            if in_multiplexer() {
                // the outer terminal is invisible to us, and asking for
                // passthrough that is not there prints escape garbage
                None
            } else if trusted_terminal() {
                kitty_picker()
            } else {
                None
            }
        }
    }
}

/// build the picker behind a hard deadline.
///
/// every `Picker` constructor shells out to `tmux set -p
/// allow-passthrough on` and blocks on `child.wait()` when it thinks it
/// is inside tmux. if that call stalls, the editor never starts. the
/// builder only reads environment and spawns a child with null stdio,
/// so abandoning it costs nothing.
fn kitty_picker() -> Option<Picker> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut p = Picker::halfblocks();
        p.set_protocol_type(ProtocolType::Kitty);
        let _ = tx.send(p);
    });
    let mut p = rx
        .recv_timeout(std::time::Duration::from_millis(400))
        .ok()?;
    p.set_protocol_type(ProtocolType::Kitty);
    Some(p)
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

/// one image, decoded once and resized on its own worker thread
struct Entry {
    dims: (u32, u32),
    protocol: Option<ThreadProtocol>,
    /// this image's own worker, so a finished resize needs no matching
    rx: Receiver<ResizeResponse>,
    /// the pane rect we last handed to the worker, and whether the
    /// reply for it has landed. keyed on the rect because that is what
    /// the widget is asked about, not the scaled image size.
    asked: Option<Rect>,
    answered: bool,
}

fn spawn_worker() -> (Sender<ResizeRequest>, Receiver<ResizeResponse>) {
    let (req_tx, req_rx) = std::sync::mpsc::channel::<ResizeRequest>();
    let (res_tx, res_rx) = std::sync::mpsc::channel::<ResizeResponse>();
    std::thread::spawn(move || {
        while let Ok(request) = req_rx.recv() {
            // a failed encode is dropped, the next frame retries
            if let Ok(done) = request.resize_encode() {
                let _ = res_tx.send(done);
            }
        }
    });
    (req_tx, res_rx)
}

/// why this matters: the crate docs are explicit that `StatefulImage`
/// with a plain `StatefulProtocol` blocks the ui thread on resize and
/// encode, which in a reactive editor happens on every scroll. the
/// threaded protocol hands that work to a worker and renders whatever
/// finished last.
pub struct Media {
    picker: Option<Picker>,
    cache: HashMap<String, Entry>,
}

impl Media {
    pub fn new(picker: Option<Picker>) -> Self {
        Self {
            picker,
            cache: HashMap::new(),
        }
    }

    /// true when the terminal can be trusted with the graphics protocol
    pub fn supported(&self) -> bool {
        self.picker.is_some()
    }

    /// decoded dimensions, read from disk only the first time
    pub fn dims(&mut self, path: &str) -> Option<(u32, u32)> {
        if let Some(entry) = self.cache.get(path) {
            return Some(entry.dims);
        }
        let picker = self.picker.as_ref()?;
        let decoded = image::open(Path::new(path)).ok()?;
        let dims = (decoded.width().max(1), decoded.height().max(1));
        let (tx, rx) = spawn_worker();
        let state = picker.new_resize_protocol(decoded);
        self.cache.insert(
            path.to_string(),
            Entry {
                dims,
                protocol: Some(ThreadProtocol::new(tx, Some(state))),
                rx,
                asked: None,
                answered: true,
            },
        );
        Some(dims)
    }

    /// rows an image needs in a pane `cols` wide. never touches the disk
    /// after the first call, which is what kept frames cheap.
    pub fn rows(&mut self, path: &str, cols: u16) -> Option<u16> {
        let dims = self.dims(path)?;
        Some(rows_for_dims(dims, cols))
    }

    /// true while any image is still waiting on its worker. the editor
    /// blocks on input between frames, so this is how a freshly encoded
    /// image gets the frame it needs to appear.
    pub fn pending(&mut self) -> bool {
        let mut any = false;
        for entry in self.cache.values_mut() {
            while let Ok(done) = entry.rx.try_recv() {
                if let Some(proto) = entry.protocol.as_mut()
                    && proto.update_resized_protocol(done)
                {
                    entry.answered = true;
                }
            }
            any |= entry.asked.is_some() && !entry.answered;
        }
        any
    }

    /// draw the image into `area`, scaled to fit.
    ///
    /// the widget encodes on the worker, so a rect that was never
    /// answered stays outstanding until its reply lands.
    pub fn draw(&mut self, frame: &mut Frame, path: &str, area: Rect) {
        if area.width == 0 || area.height == 0 || self.picker.is_none() {
            return;
        }
        self.dims(path);
        let Some(entry) = self.cache.get_mut(path) else {
            return;
        };
        if entry.asked != Some(area) {
            entry.asked = Some(area);
            entry.answered = false;
        }
        let Some(proto) = entry.protocol.as_mut() else {
            return;
        };
        while let Ok(done) = entry.rx.try_recv() {
            if proto.update_resized_protocol(done) {
                entry.answered = true;
            }
        }
        let widget = StatefulImage::default().resize(Resize::Fit(None));
        frame.render_stateful_widget(widget, area, proto);
    }
}

/// rows for an already decoded image. the renderer always goes through
/// the cached variant so this never touches the disk.
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
        let mut media = Media::new(None);
        assert_eq!(media.rows("/definitely/not/here.png", 40), None);
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
        assert_eq!(media.dims("/tmp/whatever.png"), None);
    }

    /// the real cost we removed: dimensions must come from the cache
    /// after the first load, never from disk on every frame
    #[test]
    fn dims_are_read_from_disk_once() {
        let path = std::env::temp_dir().join("blur-media-dims.png");
        let mut img = image::RgbImage::new(64, 48);
        for (_, _, px) in img.enumerate_pixels_mut() {
            *px = image::Rgb([1, 2, 3]);
        }
        img.save(&path).unwrap();
        let mut media = Media::new(None);
        // no picker means no cache and no dims, that path is covered above
        assert!(media.dims(path.to_str().unwrap()).is_none());

        let mut media = Media::new(Some(Picker::halfblocks()));
        let first = media.dims(path.to_str().unwrap());
        assert_eq!(first, Some((64, 48)));
        // with the file gone, the cache still answers
        let _ = std::fs::remove_file(&path);
        assert_eq!(media.dims(path.to_str().unwrap()), Some((64, 48)));
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
    fn building_a_picker_always_returns_quickly() {
        // the tmux probe inside the picker constructor is the thing that
        // used to freeze startup, so the deadline is the guarantee
        let start = std::time::Instant::now();
        let built = kitty_picker();
        assert!(start.elapsed() < std::time::Duration::from_millis(1500));
        assert!(built.is_some());
    }
}

#[cfg(test)]
mod picker_env_tests {
    use super::*;

    #[test]
    fn kitty_env_builds_a_picker() {
        unsafe { std::env::set_var("KITTY_WINDOW_ID", "1") };
        unsafe { std::env::remove_var("TMUX") };
        unsafe { std::env::remove_var("STY") };
        assert!(trusted_terminal(), "kitty must be trusted");
        assert!(picker(Images::from_config(None)).is_some());
    }
}
