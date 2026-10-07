mod controls;
mod helpers;
mod modes;
mod normal_mode;
mod preview;
mod select_modes;

use helpers::{Highlighter, Tab, Visual, fade, fade_rgb, fg_color, hue, tok};
use normal_mode::normal_mode;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::*;
use ratatui::text::*;
use ratatui::{DefaultTerminal, Frame};
use select_modes::{select_mode_line, select_mode1};
use unicode_width::UnicodeWidthStr;

fn main() -> std::io::Result<()> {
    ratatui::run(app)?;
    Ok(())
}

/// theme picker state: live list of every opaline theme plus a
/// cached triple of accent dots per theme for the picker rows.
struct ThemeMenu {
    names: Vec<String>,
    display: Vec<String>,
    variant: Vec<String>,
    dots: Vec<Option<[(u8, u8, u8); 3]>>,
    sel: usize,
}

/// one preview row: its line index, the parsed row, and the loaded
/// media behind it when the row is an image.
type PreviewItem = (usize, preview::DocRow, Option<(u32, u16)>);

/// resolve an image reference against the document directory. absolute
/// and ~/ paths are used as-is so real files are never mangled into a
/// "missing" lookup.
fn resolve_media_path(path: &str, dir: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        match std::env::var("HOME") {
            Ok(h) => return format!("{h}/{rest}"),
            Err(_) => return path.to_string(),
        }
    }
    if path.starts_with('/') {
        return path.to_string();
    }
    format!("{dir}/{path}")
}

fn load_theme_robust(name: &str) -> Option<opaline::Theme> {
    if let Some(t) = opaline::load_by_name(name) {
        return Some(t);
    }
    for item in opaline::list_available_themes() {
        if (item.name.eq_ignore_ascii_case(name) || item.display_name.eq_ignore_ascii_case(name))
            && let Some(t) = opaline::load_by_name(&item.name) {
                return Some(t);
            }
    }
    None
}

/// returns false when the theme fails to load, so callers can
/// retry or stay put instead of silently keeping a half state.
fn apply_theme(
    theme: &mut opaline::Theme,
    highlighter: &mut Highlighter,
    tabs: &mut [Tab],
    name: &str,
) -> bool {
    if let Some(t) = load_theme_robust(name) {
        *theme = t;
        highlighter.set_theme(theme);
        for tab in tabs.iter_mut() {
            tab.highlight_cache = None;
        }
        true
    } else {
        false
    }
}

fn fill_dots(menu: &mut ThemeMenu) {
    for (i, name) in menu.names.iter().enumerate() {
        if menu.dots[i].is_some() {
            continue;
        }
        menu.dots[i] = load_theme_robust(name).and_then(|t| {
            let c = |k: &str| t.try_color(k).map(|c| (c.r, c.g, c.b));
            Some([
                c("accent.primary")?,
                c("accent.secondary")?,
                c("accent.tertiary")?,
            ])
        });
    }
}

/// settings live in $XDG_CONFIG_HOME/blur/config.toml (or
/// ~/.config/blur/config.toml). tiny hand parser, no new deps.
/// failures fall back to defaults and never crash the editor.
fn config_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
        })?;
    Some(base.join("blur").join("config.toml"))
}

/// tiny reader for the flat config file. unknown keys and bad values
/// are ignored rather than fatal, so a typo never breaks startup.
fn load_config() -> (Option<String>, bool) {
    let mut theme = None;
    let mut images = preview::images_enabled();
    let text = match config_path().and_then(|p| std::fs::read_to_string(p).ok()) {
        Some(t) => t,
        None => return (None, images),
    };
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        match key.trim() {
            "theme" => {
                if opaline::load_by_name(value).is_some() {
                    theme = Some(value.to_string());
                }
            }
            "images" => {
                images = matches!(value, "1" | "true" | "on");
            }
            _ => {}
        }
    }
    (theme, images)
}

fn save_config_theme(name: &str) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, format!("theme = \"{}\"\n", name));
    }
}

/// shape the terminal cursor to match what the editor is doing: bar while
/// typing, underline while selecting, block at rest.
fn cursor_shape(mode: i32) -> crossterm::cursor::SetCursorStyle {
    match mode {
        1 | 10 | 11 => crossterm::cursor::SetCursorStyle::BlinkingBar,
        2 | 3 => crossterm::cursor::SetCursorStyle::SteadyUnderScore,
        _ => crossterm::cursor::SetCursorStyle::SteadyBlock,
    }
}

/// close the current tab. returns true when the app should exit.
fn drop_current_tab(tabs: &mut Vec<Tab>, tab_selector: &mut usize) -> bool {
    if tabs.len() > 1 {
        tabs.remove(*tab_selector);
        if *tab_selector >= tabs.len() {
            *tab_selector = tabs.len() - 1;
        }
        false
    } else {
        true
    }
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    crossterm::execute!(
        std::io::stdout(),
        crossterm::event::EnableBracketedPaste,
        crossterm::event::EnableMouseCapture,
    )?;
    let args: Vec<String> = std::env::args().collect();
    let (cfg_theme, cfg_images) = load_config();
    let mut theme_name = cfg_theme.unwrap_or_else(|| String::from("catppuccin-mocha"));
    let mut theme = opaline::load_by_name(&theme_name).unwrap();
    let mut theme_prev = theme_name.clone();

    let listed = opaline::list_available_themes();
    let mut menu = ThemeMenu {
        names: listed.iter().map(|t| t.name.clone()).collect(),
        display: listed.iter().map(|t| t.display_name.clone()).collect(),
        variant: listed
            .iter()
            .map(|t| format!("{}", t.variant).to_lowercase())
            .collect(),
        dots: vec![None; listed.len()],
        sel: 0,
    };

    let mut tabs = Vec::new();
    let mut tab_selector: usize = 0;
    let mut highlighter = Highlighter::new(&theme);
    let mut vis = Visual::new();
    let mut mode = 0;
    let mut the_command_line = String::new();
    let mut filled_now = String::new();
    let mut pv = preview::PreviewState::new();
    pv.images = cfg_images;
    let mut layout = preview::UiLayout::default();
    let mut confirm_all = false;
    let mut press: Option<(i32, i32)> = None;
    let mut last_shape: Option<crossterm::cursor::SetCursorStyle> = None;
    tabs.push(Tab::new());
    match args.len() {
        1 => {}
        _ => match std::fs::read_to_string(&args[1]) {
            Ok(content) => {
                tabs[0].input_box = content.split('\n').map(|line| line.to_string()).collect();
                tabs[0].file_name = args[1].clone();
                tabs[0].saved = true;
            }
            Err(_) => {
                tabs[0].input_box = vec![String::new()];
                tabs[0].file_name = args[1].clone();
            }
        },
    }
    loop {
        // cursor shape belongs to the terminal, not the frame. write it
        // only when it actually changes, always outside the draw call.
        let shape = cursor_shape(mode);
        if last_shape != Some(shape) {
            let _ = crossterm::execute!(std::io::stdout(), shape);
            last_shape = Some(shape);
        }
        let (before, rest) = tabs.split_at_mut(tab_selector);
        let (active, after) = rest.split_first_mut().unwrap();
        terminal.draw(|frame| {
            renderer(
                frame,
                &theme,
                before,
                after,
                active,
                &highlighter,
                &vis,
                &menu,
                &theme_name,
                &mut pv,
                &mut layout,
                confirm_all,
                mode,
                &the_command_line,
            )
        })?;
        let tab = &mut tabs[tab_selector];

        let event = crossterm::event::read()?;
        let mut the_text = tab.input_box.clone();
        match &event {
            crossterm::event::Event::Paste(text) => match mode {
                1 => modes::insert_paste(tab, text),
                10 | 11 => the_command_line.push_str(text),
                _ => {}
            },
            // placements are absolute cells, drop them on resize
            crossterm::event::Event::Resize(_, _) => {
                preview::clear_media(&mut pv);
            }
            crossterm::event::Event::Mouse(m) => {
                use crossterm::event::{MouseButton, MouseEventKind};
                let (cx, cy) = (m.column, m.row);
                layout.mouse = Some((cx, cy));
                let hit =
                    |r: Rect| cx >= r.x && cx < r.x + r.width && cy >= r.y && cy < r.y + r.height;
                if mode == 403 {
                    // left half confirms, anywhere else cancels. never
                    // destructive on an accidental click outside.
                    if matches!(m.kind, MouseEventKind::Down(MouseButton::Left)) {
                        let yes = layout
                            .modal
                            .map(|a| hit(a) && cx < a.x + a.width / 2)
                            .unwrap_or(false);
                        if yes {
                            if confirm_all {
                                break;
                            }
                            if drop_current_tab(&mut tabs, &mut tab_selector) {
                                break;
                            }
                            mode = 0;
                        } else {
                            confirm_all = false;
                            mode = 0;
                        }
                    }
                } else if mode == 12 {
                    if matches!(m.kind, MouseEventKind::Down(MouseButton::Left))
                        && let Some((area, top, rows)) = layout.picker
                        && hit(area)
                        && cy > area.y
                        && cy < area.y + area.height - 1
                    {
                        let r = (cy - area.y - 1) as usize;
                        if r < rows && !menu.names.is_empty() {
                            menu.sel = (top + r).min(menu.names.len() - 1);
                            if let Some(name) = menu.names.get(menu.sel).cloned()
                                && apply_theme(&mut theme, &mut highlighter, &mut tabs, &name)
                            {
                                theme_name = name;
                                save_config_theme(&theme_name);
                                mode = 0;
                            }
                        }
                    }
                } else if matches!(mode, 401 | 402) {
                    if matches!(m.kind, MouseEventKind::Down(_)) {
                        mode = if mode == 401 { 11 } else { 10 };
                    }
                } else if matches!(mode, 0..=3) {
                    if cy == layout.tab_y {
                        // the close cell wins over selecting the tab, and
                        // middle click anywhere on a pill closes it
                        let on_close = layout
                            .tab_close
                            .iter()
                            .find(|(x0, x1, _)| cx >= *x0 && cx < *x1)
                            .map(|(_, _, i)| *i);
                        let middle = matches!(m.kind, MouseEventKind::Down(MouseButton::Middle));
                        let left = matches!(m.kind, MouseEventKind::Down(MouseButton::Left));
                        let target = if left {
                            on_close.or_else(|| {
                                layout
                                    .tab_pills
                                    .iter()
                                    .find(|(x0, x1, _)| cx >= *x0 && cx < *x1)
                                    .map(|(_, _, i)| *i)
                            })
                        } else if middle {
                            layout
                                .tab_pills
                                .iter()
                                .find(|(x0, x1, _)| cx >= *x0 && cx < *x1)
                                .map(|(_, _, i)| *i)
                        } else {
                            None
                        };
                        if let Some(mut i) = target {
                            if Some(i) == on_close || middle {
                                if !tabs[i].saved {
                                    confirm_all = false;
                                    mode = 403;
                                } else if drop_current_tab(&mut tabs, &mut i) {
                                    break;
                                } else {
                                    tab_selector = tab_selector.min(tabs.len() - 1);
                                    mode = 0;
                                }
                            } else {
                                tab_selector = i;
                            }
                            press = None;
                        }
                    } else {
                        let inner = layout.edit_inner;
                        let edit_hit = hit(inner);
                        let prev_hit = layout.prev_inner.map(hit).unwrap_or(false);
                        let place = |t: &mut Tab| {
                            preview::cell_to_buffer(
                                &t.input_box,
                                inner.x,
                                layout.gutter_w,
                                t.scroll_x,
                                t.scroll_y,
                                cx,
                                cy,
                                inner.y,
                            )
                        };
                        match m.kind {
                            MouseEventKind::Down(MouseButton::Left) => {
                                if edit_hit {
                                    let t = &mut tabs[tab_selector];
                                    let (ry, rx) = place(t);
                                    t.cursor_y = ry;
                                    t.cursor_x = rx;
                                    press = Some((ry, rx));
                                } else {
                                    press = None;
                                }
                            }
                            MouseEventKind::Drag(MouseButton::Left) => {
                                if edit_hit {
                                    if mode == 0
                                        && let Some((py, px)) = press
                                    {
                                        vis.v_x = px.max(0) as usize;
                                        vis.v_y = py.max(0) as usize;
                                        vis.on = true;
                                        mode = 2;
                                    }
                                    if mode == 2 || mode == 3 {
                                        let t = &mut tabs[tab_selector];
                                        let (ry, rx) = place(t);
                                        t.cursor_y = ry;
                                        t.cursor_x = rx;
                                    }
                                }
                            }
                            MouseEventKind::Up(_) => {
                                press = None;
                            }
                            MouseEventKind::ScrollUp => {
                                if edit_hit || prev_hit {
                                    let t = &mut tabs[tab_selector];
                                    t.scroll_y = t.scroll_y.saturating_sub(3);
                                }
                            }
                            MouseEventKind::ScrollDown if (edit_hit || prev_hit) => {
                                let t = &mut tabs[tab_selector];
                                t.scroll_y = t.scroll_y.saturating_add(3);
                            }
                            _ => {}
                        }
                    }
                }
            }
            crossterm::event::Event::Key(event_key) => {
                match mode {
                    0 => {
                        // preview pane and theme picker live here so
                        // normal_mode stays edit-only
                        if event_key.code == crossterm::event::KeyCode::Char('P') {
                            pv.open = !pv.open;
                            if !pv.open {
                                preview::clear_media(&mut pv);
                            }
                        } else if event_key.code == crossterm::event::KeyCode::Char('t') {
                            menu.sel = menu
                                .names
                                .iter()
                                .position(|n| {
                                    n == &theme_name || n.eq_ignore_ascii_case(&theme_name)
                                })
                                .or_else(|| {
                                    menu.display
                                        .iter()
                                        .position(|d| d.eq_ignore_ascii_case(&theme_name))
                                })
                                .unwrap_or(menu.sel);
                            theme_prev = theme_name.clone();
                            fill_dots(&mut menu);
                            mode = 12;
                        } else if event_key.code == crossterm::event::KeyCode::Char('Q') {
                            // quit everything, confirming when anything is unsaved
                            if tabs.iter().any(|t| !t.saved) {
                                confirm_all = true;
                                mode = 403;
                            } else {
                                break;
                            }
                        } else if event_key.code == crossterm::event::KeyCode::Char('X') {
                            // close all saved tabs, keep working where unsaved
                            let mut i = 0;
                            while i < tabs.len() {
                                if tabs[i].saved {
                                    tabs.remove(i);
                                } else {
                                    i += 1;
                                }
                            }
                            if tabs.is_empty() {
                                tabs.push(Tab::new());
                            }
                            tab_selector = tab_selector.min(tabs.len() - 1);
                            mode = 0;
                        } else if !normal_mode(
                            &mut tabs,
                            &mut tab_selector,
                            &mut vis,
                            *event_key,
                            &mut mode,
                            &mut the_command_line,
                            &mut the_text,
                        )
                        .unwrap()
                        {
                            if drop_current_tab(&mut tabs, &mut tab_selector) {
                                break;
                            }
                            mode = 0;
                        }
                    }
                    1 => {
                        if !modes::insert_mode(
                            tab,
                            *event_key,
                            &mut mode,
                            &mut the_text,
                            &mut filled_now,
                        )
                        .unwrap()
                        {
                            continue;
                        }
                    }
                    2 => {
                        if !select_mode1(tab, &mut vis, *event_key, &mut mode).unwrap() {
                            mode = 0;
                            continue;
                        }
                    }
                    3 => {
                        if !select_mode_line(tab, &mut vis, *event_key, &mut mode).unwrap() {
                            mode = 0;
                            continue;
                        }
                    }

                    10 => {
                        if !modes::save_mode(tab, *event_key, &mut the_command_line, &mut mode)
                            .unwrap()
                        {
                            mode = 402;
                        }
                    }
                    11 => {
                        if !modes::open_mode(
                            &mut tabs,
                            &mut tab_selector,
                            *event_key,
                            &mut the_command_line,
                            &mut mode,
                        )
                        .unwrap()
                        {
                            mode = 401;
                        }
                    }

                    403 => {
                        if !modes::unsaved_work_mode(*event_key, &mut mode).unwrap() {
                            if confirm_all {
                                break;
                            }
                            if drop_current_tab(&mut tabs, &mut tab_selector) {
                                break;
                            }
                            mode = 0;
                        } else if mode == 0 {
                            confirm_all = false;
                        }
                    }
                    // theme picker: move with live preview, enter keeps, esc restores
                    12 => {
                        let n = menu.names.len();
                        let preview = |menu: &ThemeMenu,
                                       theme: &mut opaline::Theme,
                                       highlighter: &mut Highlighter,
                                       tabs: &mut [Tab]| {
                            if let Some(name) = menu.names.get(menu.sel) {
                                apply_theme(theme, highlighter, tabs, name);
                            }
                        };
                        match event_key.code {
                            crossterm::event::KeyCode::Char('j')
                            | crossterm::event::KeyCode::Down => {
                                if n > 0 {
                                    menu.sel = (menu.sel + 1) % n;
                                    preview(&menu, &mut theme, &mut highlighter, &mut tabs);
                                }
                            }
                            crossterm::event::KeyCode::Char('k')
                            | crossterm::event::KeyCode::Up => {
                                if n > 0 {
                                    menu.sel = (menu.sel + n - 1) % n;
                                    preview(&menu, &mut theme, &mut highlighter, &mut tabs);
                                }
                            }
                            crossterm::event::KeyCode::Enter => {
                                // re-apply explicitly so enter never depends
                                // on preview state. on failure the picker
                                // stays open instead of going half broken.
                                if let Some(name) = menu.names.get(menu.sel).cloned() {
                                    if apply_theme(&mut theme, &mut highlighter, &mut tabs, &name) {
                                        theme_name = name;
                                        save_config_theme(&theme_name);
                                        mode = 0;
                                    }
                                } else {
                                    mode = 0;
                                }
                            }
                            crossterm::event::KeyCode::Esc => {
                                let back = theme_prev.clone();
                                apply_theme(&mut theme, &mut highlighter, &mut tabs, &back);
                                theme_name = back;
                                mode = 0;
                            }
                            _ => {}
                        }
                    }
                    // error banners: any key returns to the prompt for a retry
                    401 => {
                        mode = 11;
                    }
                    402 => {
                        mode = 10;
                    }
                    _ => {
                        the_command_line.clear();
                        mode = 0;
                    }
                }
            }
            _ => {}
        }
    }
    preview::clear_media(&mut pv);
    crossterm::execute!(
        std::io::stdout(),
        crossterm::event::DisableBracketedPaste,
        crossterm::event::DisableMouseCapture,
        crossterm::cursor::SetCursorStyle::DefaultUserShape,
    )?;
    Ok(())
}

fn mode_style(mode: i32, theme: &opaline::Theme) -> (String, Style, Color) {
    let (label, key) = match mode {
        0 => (" normal ", "blue"),
        1 => (" insert ", "green"),
        2 => (" visual ", "mauve"),
        3 => (" v-line ", "pink"),
        10 | 11 => (" command ", "peach"),
        12 => (" themes ", "teal"),
        401 | 402 => (" error ", "red"),
        403 => (" confirm ", "yellow"),
        _ => (" error ", "red"),
    };
    let bg: Color = hue(theme, key).into();
    (
        label.to_string(),
        Style::default().fg(fg_color(hue(theme, key))).bg(bg),
        bg,
    )
}

fn short_of(path: &str) -> String {
    if path.is_empty() {
        "untitled".to_string()
    } else {
        path.rsplit('/').next().unwrap_or(path).to_lowercase()
    }
}

// nvim-web-devicons style map, colors land on catppuccin keys
fn file_icon(path: &str) -> (&'static str, &'static str) {
    let l = path.to_lowercase();
    if l.ends_with(".rs") {
        (" ", "peach")
    } else if l.ends_with(".toml") || l.ends_with(".lock") {
        (" ", "red")
    } else if l.ends_with(".py") {
        (" ", "green")
    } else if l.ends_with(".js") || l.ends_with(".mjs") {
        (" ", "yellow")
    } else if l.ends_with(".ts") || l.ends_with(".tsx") {
        (" ", "blue")
    } else if l.ends_with(".json") {
        (" ", "yellow")
    } else if l.ends_with(".md") || l.ends_with(".markdown") {
        (" ", "sapphire")
    } else if l.ends_with(".sh")
        || l.ends_with(".bash")
        || l.ends_with(".zsh")
        || l.ends_with(".fish")
    {
        (" ", "green")
    } else if l.ends_with(".c") || l.ends_with(".h") {
        (" ", "blue")
    } else if l.ends_with(".cpp") || l.ends_with(".hpp") || l.ends_with(".cc") {
        (" ", "mauve")
    } else if l.ends_with(".lua") {
        (" ", "blue")
    } else if l.ends_with(".go") {
        (" ", "teal")
    } else if l.ends_with(".html") || l.ends_with(".htm") {
        (" ", "peach")
    } else if l.ends_with(".css") || l.ends_with(".scss") {
        (" ", "sky")
    } else if l.ends_with(".yaml") || l.ends_with(".yml") {
        ("󰈙 ", "pink")
    } else if l.ends_with(".sql") {
        (" ", "teal")
    } else if l.ends_with(".gitignore") || l.ends_with(".git") {
        (" ", "red")
    } else if path.is_empty() {
        ("󰈙 ", "overlay0")
    } else {
        ("󰈙 ", "subtext0")
    }
}

fn mode_icon(mode: i32) -> &'static str {
    match mode {
        0 => "󰘳 ",
        1 => "󰏫 ",
        2 => "󰈈 ",
        3 => "󰒅 ",
        10 | 11 => "󰞷 ",
        12 => " ",
        401 | 402 => "󰅚 ",
        403 => " ",
        _ => "󰘳 ",
    }
}

/// notepad style tab names: duplicate basenames gain just enough
/// parent path to stay unique, long names shorten in the middle.
fn tab_display_names(paths: &[&str]) -> Vec<String> {
    let bases: Vec<String> = paths.iter().map(|p| short_of(p)).collect();
    let mut out = bases.clone();
    for i in 0..paths.len() {
        if paths[i].is_empty() {
            continue;
        }
        let dupes: Vec<usize> = bases
            .iter()
            .enumerate()
            .filter(|(j, b)| *j != i && **b == bases[i])
            .map(|(j, _)| j)
            .collect();
        if dupes.is_empty() {
            continue;
        }
        let segs: Vec<&str> = paths[i].split('/').collect();
        let mut depth = 2;
        while depth <= segs.len() {
            let cand = segs[segs.len().saturating_sub(depth)..].join("/");
            let cand = cand.to_lowercase();
            let clash = dupes.iter().any(|j| {
                let o: Vec<&str> = paths[*j].split('/').collect();
                o[o.len().saturating_sub(depth)..].join("/").to_lowercase() == cand
            });
            out[i] = cand;
            if !clash {
                break;
            }
            depth += 1;
        }
    }
    out
}

/// shorten to max chars with a middle ellipsis, cells matter not bytes.
fn shorten_middle(name: &str, max: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max || max <= 2 {
        return name.to_string();
    }
    let keep = max - 1;
    let head = keep * 2 / 3;
    let tail = keep - head;
    format!(
        "{}…{}",
        chars[..head].iter().collect::<String>(),
        chars[chars.len() - tail..].iter().collect::<String>()
    )
}

fn hint_for(mode: i32) -> &'static str {
    match mode {
        0 => "i insert · v visual · t themes · P preview · q quit",
        1 => "esc normal",
        2 | 3 => "d delete · esc cancel",
        10 | 11 => "enter ok · esc cancel",
        12 => "j/k move · enter apply · esc cancel",
        403 => "y quit · n stay",
        _ => "esc back",
    }
}

#[allow(clippy::too_many_arguments)]
fn renderer(
    frame: &mut Frame,
    theme: &opaline::Theme,
    before: &[Tab],
    after: &[Tab],
    tab: &mut Tab,
    highlighter: &Highlighter,
    vis: &Visual,
    menu: &ThemeMenu,
    theme_name: &str,
    pv: &mut preview::PreviewState,
    lay: &mut preview::UiLayout,
    quit_all: bool,
    mode: i32,
    the_command_line: &str,
) {
    use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

    let base: Color = Color::Reset;
    let dim: Color = tok(theme, "text.dim").into();
    let muted: Color = tok(theme, "text.muted").into();
    let textc: Color = tok(theme, "text.primary").into();
    let faint: Color = tok(theme, "border.unfocused").into();
    let lav: Color = tok(theme, "accent.deep").into();
    let errc: Color = tok(theme, "error").into();

    let areas = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());
    let (tab_area, edit_area, status_area) = (areas[0], areas[1], areas[2]);

    // side preview for markdown when the screen is wide enough
    let lower = tab.file_name.to_lowercase();
    let is_md = lower.ends_with(".md") || lower.ends_with(".markdown") || lower.ends_with(".mdown");
    let split = pv.open && is_md && edit_area.width >= 70;
    let (code_outer, prev_outer) = if split {
        let cols = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(edit_area);
        (cols[0], Some(cols[1]))
    } else {
        (edit_area, None)
    };

    // scroll, cursor rests near top third (inside rounded box)
    let edit_h = code_outer.height.saturating_sub(2);
    let margin = (edit_h / 3).max(1);
    if (tab.cursor_y as u16) < tab.scroll_y + margin {
        tab.scroll_y = (tab.cursor_y as u16).saturating_sub(margin);
    } else if (tab.cursor_y as u16) >= tab.scroll_y + edit_h.saturating_sub(margin) {
        tab.scroll_y =
            (tab.cursor_y as u16).saturating_sub(edit_h.saturating_sub(margin).saturating_sub(1));
    }

    let empty = String::new();
    let current_line = tab.input_box.get(tab.cursor_y as usize).unwrap_or(&empty);
    let visual_x = current_line
        .get(..tab.cursor_x as usize)
        .map(UnicodeWidthStr::width)
        .unwrap_or(tab.cursor_x as usize) as u16;

    let total = tab.input_box.len().max(1);
    let digits = total.to_string().len().max(2) as u16;
    // gutter renders as `{num:>digits} │ `, exactly digits + 3 cells, so the
    // divider always keeps one fixed space before the file content
    let gutter_w = digits + 3;
    let text_w = code_outer.width.saturating_sub(gutter_w + 4);
    if visual_x <= tab.scroll_x {
        tab.scroll_x = visual_x;
    } else if visual_x >= tab.scroll_x + text_w {
        tab.scroll_x = visual_x - text_w + 1;
    }

    let (label, mstyle, mcolor) = mode_style(mode, theme);

    // top: yazi style pill tabs, active filled in mode color
    let mode_key = match mode {
        0 => "blue",
        1 => "green",
        2 => "mauve",
        3 => "pink",
        10 | 11 => "peach",
        401 | 402 => "red",
        403 => "yellow",
        _ => "blue",
    };
    // top: pill tabs with notepad shortening, disambiguated dupes,
    // and a scrolling window pinned on the active tab. hint on the right.
    let mut all: Vec<&Tab> = before.iter().collect();
    all.push(tab);
    all.extend(after.iter());
    let active_idx = before.len();
    let paths: Vec<&str> = all.iter().map(|t| t.file_name.as_str()).collect();
    let names = tab_display_names(&paths);
    // pill: index badge, file icon, name, state dot, close affordance.
    // the close cell is always reserved so hovering never shifts layout.
    let hover = lay.mouse;
    let _ = hover;
    // always show '×' even for a single tab
    let pills: Vec<(Vec<Span>, usize, u16)> = all
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let active = i == active_idx;
            let name = shorten_middle(&names[i], 22);
            let (glyph, _) = file_icon(&t.file_name);
            let mut spans: Vec<Span> = Vec::new();
            let mut w = 0usize;
            let close_at;
            if active {
                let fg = fg_color(hue(theme, mode_key));
                // Active tab: sharp but colorful pill
                spans.push(Span::styled(" ", Style::default().bg(base)));
                w += 1;
                spans.push(Span::styled("", Style::default().fg(mcolor).bg(base)));
                w += 1;
                spans.push(Span::styled(
                    format!(" {} ", i + 1),
                    Style::default().fg(fg).bg(mcolor).bold(),
                ));
                w += 3;
                spans.push(Span::styled(glyph, Style::default().fg(fg).bg(mcolor)));
                w += UnicodeWidthStr::width(glyph);
                spans.push(Span::styled(
                    format!("{} ", name),
                    Style::default().fg(fg).bg(mcolor).bold(),
                ));
                w += UnicodeWidthStr::width(name.as_str()) + 1;
                if !t.saved {
                    spans.push(Span::styled("● ", Style::default().fg(errc).bg(mcolor)));
                    w += 2;
                }
                close_at = w as u16;
                spans.push(Span::styled(
                    "× ",
                    Style::default().fg(fg).bg(mcolor).bold(),
                ));
                w += 2;
                spans.push(Span::styled("", Style::default().fg(mcolor).bg(base)));
                w += 1;
            } else {
                // Inactive tab: very minimal, no background, just dim text and icons
                spans.push(Span::styled(
                    format!("  {} ", i + 1),
                    Style::default().fg(faint).bg(base),
                ));
                w += 4;
                spans.push(Span::styled(glyph, Style::default().fg(dim).bg(base)));
                w += UnicodeWidthStr::width(glyph);
                spans.push(Span::styled(
                    name.clone(),
                    Style::default().fg(dim).bg(base),
                ));
                w += UnicodeWidthStr::width(name.as_str());
                if !t.saved {
                    spans.push(Span::styled(" ●", Style::default().fg(errc).bg(base)));
                    w += 2;
                }
                spans.push(Span::styled(" ", Style::default().bg(base)));
                w += 1;
                close_at = w as u16;
                spans.push(Span::styled("× ", Style::default().fg(faint).bg(base)));
                w += 2;
            }
            (spans, w, close_at)
        })
        .collect();

    let hint = hint_for(mode);
    let hint_w = UnicodeWidthStr::width(hint);
    // 1 leading space + 2 overflow markers + hint gap
    let budget = (tab_area.width as usize).saturating_sub(hint_w + 5);
    // window tabs around the active one so it is always visible
    let mut lo = active_idx;
    let mut hi_excl = active_idx + 1;
    let mut used_w = pills[active_idx].1;
    while lo > 0 || hi_excl < pills.len() {
        let mut grew = false;
        if lo > 0 && used_w + pills[lo - 1].1 <= budget {
            lo -= 1;
            used_w += pills[lo].1;
            grew = true;
        }
        if hi_excl < pills.len() && used_w + pills[hi_excl].1 <= budget {
            used_w += pills[hi_excl].1;
            hi_excl += 1;
            grew = true;
        }
        if !grew {
            break;
        }
    }
    let mut tabline = Line::from(vec![Span::styled(" ", Style::default().bg(base))]);
    lay.tab_pills.clear();
    lay.tab_y = tab_area.y;
    lay.prev_inner = None;
    lay.picker = None;
    let mut pill_x = tab_area.x + 1;
    if lo > 0 {
        tabline
            .spans
            .push(Span::styled("‹ ", Style::default().fg(dim).bg(base)));
        pill_x += 2;
    }
    lay.tab_close.clear();
    for (k, (spans, w, close_at)) in pills.iter().enumerate().take(hi_excl).skip(lo) {
        lay.tab_pills.push((pill_x, pill_x + *w as u16, k));
        let cx = pill_x + close_at;
        lay.tab_close.push((cx, cx + 2, k));
        pill_x += *w as u16;
        tabline.spans.extend(spans.clone());
    }
    if hi_excl < pills.len() {
        tabline
            .spans
            .push(Span::styled(" ›", Style::default().fg(dim).bg(base)));
    }
    let used: usize = tabline
        .spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
        .sum();
    let avail = (tab_area.width as usize).saturating_sub(used + hint_w + 1);
    tabline.spans.push(Span::raw(" ".repeat(avail)));
    tabline
        .spans
        .push(Span::styled(hint, Style::default().fg(faint).bg(base)));
    frame.render_widget(Paragraph::new(tabline).bg(base), tab_area);

    // middle: gutter + code, zero tinted bg, cursor line reads via
    // bold number + underline, selection via bold + underline. transparent.
    // subtle wash so the cursor line reads without hiding the theme
    let mut visible: Vec<Line> = highlighter
        .highlight(tab)
        .into_iter()
        .enumerate()
        .skip(tab.scroll_y as usize)
        .take(edit_h as usize)
        .map(|(i, mut line)| {
            let cur = i as i32 == tab.cursor_y;
            let in_selection = if vis.on && (mode == 2 || mode == 3) {
                if mode == 3 {
                    (i as i32 - vis.v_y as i32).abs() + (vis.v_y as i32 - tab.cursor_y).abs()
                        == (i as i32 - tab.cursor_y).abs()
                } else {
                    let (y1, y2) = if tab.cursor_y < vis.v_y as i32 {
                        (tab.cursor_y, vis.v_y as i32)
                    } else {
                        (vis.v_y as i32, tab.cursor_y)
                    };
                    i as i32 >= y1 && i as i32 <= y2
                }
            } else {
                false
            };

            let num = format!("{:>w$} │ ", i + 1, w = digits as usize);
            let num_st = if cur {
                Style::default().fg(lav).bold()
            } else if in_selection {
                Style::default().fg(textc).bold()
            } else {
                Style::default().fg(faint)
            };

            let mut spans = vec![Span::styled(num, num_st)];
            if cur {
                // subtly highlight the gutter symbol too
                spans[0].style = spans[0].style.fg(mcolor);
            }
            // a whisper of wash on the cursor line, none while typing so
            // the code area stays transparent in insert mode
            let wash = (cur && mode != 1).then(|| fade(theme, tok(theme, "accent.deep"), 0.09));
            if let Some(bg) = wash {
                for s in &mut line.spans {
                    s.style = s.style.bg(bg);
                }
            }
            for s in &mut line.spans {
                if cur {
                    s.style = s.style.add_modifier(Modifier::UNDERLINED);
                }
                if in_selection {
                    // For mode 2 (visual), we should ideally highlight specific characters,
                    // but for now let's at least highlight the whole line or parts of it.
                    // Improving visual mode to character-level would require more complex span splitting.
                    s.style = s.style.bg(fade_rgb(theme, lav, 0.3));
                }
            }

            spans.append(&mut line.spans);
            Line::from(spans)
        })
        .collect();

    while visible.len() < edit_h as usize {
        visible.push(Line::from(Span::styled(" ", Style::default().bg(base))));
    }

    // calm dim frame so the interior stays deep, mode color lives
    // only on pills, thumb, and cursor number. bright frame sides
    // against a dark interior read as a rendering bug, so never that.
    let focus = mcolor;
    let frame_col: Color = tok(theme, "border.unfocused").into();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(frame_col))
        .style(Style::default().bg(base))
        // title shows the parent dir, the name already lives in the tabline.
        // bare filenames show the name itself, never a wrong "untitled".
        .title_top({
            let dir = tab
                .file_name
                .rsplit_once('/')
                .map(|(d, _)| {
                    if d.is_empty() {
                        "/".to_string()
                    } else {
                        d.to_string()
                    }
                })
                .unwrap_or_else(|| {
                    if tab.file_name.is_empty() {
                        "untitled".to_string()
                    } else {
                        short_of(&tab.file_name)
                    }
                });
            Line::from(vec![
                Span::styled(" ", Style::default().bg(base)),
                Span::styled("", Style::default().fg(focus).bg(base)),
                Span::styled(
                    " ",
                    Style::default()
                        .fg(fg_color(hue(theme, mode_key)))
                        .bg(focus),
                ),
                Span::styled(
                    dir,
                    Style::default()
                        .fg(fg_color(hue(theme, mode_key)))
                        .bg(focus),
                ),
                Span::styled(
                    " ",
                    Style::default()
                        .fg(fg_color(hue(theme, mode_key)))
                        .bg(focus),
                ),
                Span::styled("", Style::default().fg(focus).bg(base)),
            ])
        })
        .title_alignment(Alignment::Right);
    let inner = block.inner(code_outer);
    frame.render_widget(block, code_outer);
    lay.edit_inner = inner;
    lay.gutter_w = gutter_w;

    let code_w = inner.width.saturating_sub(1);
    if total > 1 && total as u16 > inner.height && code_w > 0 && inner.height > 0 {
        let h = inner.height as usize;
        let pos = (tab.cursor_y as usize * h.saturating_sub(1) / total.saturating_sub(1))
            .min(h.saturating_sub(1));
        for row in 0..h {
            let active = row == pos;
            // half block so the thumb reads as a scrollbar and never gets
            // mistaken for a second caret on the cursor row
            let ch = if active { "▐" } else { "·" };
            let st = if active {
                Style::default().fg(focus).bg(base)
            } else {
                Style::default().fg(faint).bg(base)
            };
            frame.render_widget(
                Paragraph::new(Span::styled(ch, st)).bg(base),
                Rect::new(inner.x + code_w, inner.y + row as u16, 1, 1),
            );
        }
    }
    frame.render_widget(
        Paragraph::new(Text::from(visible))
            .bg(base)
            .scroll((0, tab.scroll_x)),
        Rect::new(inner.x, inner.y, code_w, inner.height),
    );

    // right: live readme preview in its own rounded box. markdown parses
    // every frame so edits show instantly, images draw via kitty gfx.
    if let Some(prev_area) = prev_outer {
        let mdc = preview::MdColors {
            text: textc,
            dim,
            faint,
            accent: lav,
            code: hue(theme, "green").into(),
            quote: dim,
        };
        let doc = preview::parse_markdown(&tab.input_box, &mdc);
        let dir = tab
            .file_name
            .rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_else(|| ".".to_string());
        let media_total = doc
            .iter()
            .filter(|r| matches!(r, preview::DocRow::Media { .. }))
            .count();
        let pblock = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(frame_col))
            .style(Style::default().bg(base))
            .title_top(Line::from(vec![
                Span::styled(" ", Style::default().bg(base)),
                Span::styled("", Style::default().fg(focus).bg(base)),
                Span::styled(
                    if media_total > 0 {
                        format!(" preview · {} img ", media_total)
                    } else {
                        " preview ".to_string()
                    },
                    Style::default()
                        .fg(fg_color(hue(theme, mode_key)))
                        .bg(focus)
                        .bold(),
                ),
                Span::styled("", Style::default().fg(focus).bg(base)),
            ]))
            .title_alignment(Alignment::Right);
        let pinner = pblock.inner(prev_area);
        frame.render_widget(pblock, prev_area);
        lay.prev_inner = Some(pinner);

        let cols = pinner.width;
        // pass 1: resolve every media ref so row counts are known, then
        // scroll proportionally like the editor. pass 2 draws the window.
        let mut items: Vec<PreviewItem> = Vec::new();
        let mut row_no = 0usize;
        for row in doc {
            let media = match &row {
                preview::DocRow::Media { path, .. } => {
                    let full = resolve_media_path(path, &dir);
                    let ok = !full.is_empty()
                        && std::path::Path::new(&full).exists()
                        && pv.images
                        && cols > 4;
                    if ok {
                        preview::ensure_media(pv, &full, cols)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let height = media.map(|(_, h)| h as usize).unwrap_or(1);
            items.push((row_no, row, media));
            row_no += height;
        }
        let doc_rows = row_no;
        let view_h = pinner.height as usize;
        let denom = total.saturating_sub(inner.height as usize).max(1);
        let frac = (tab.scroll_y as usize).min(denom) as f64 / denom as f64;
        let start = (frac * doc_rows.saturating_sub(view_h) as f64) as usize;
        let end = start.saturating_add(view_h);

        let mut slice: Vec<Line> = Vec::with_capacity(view_h);
        let mut cursor_row = 0usize;
        let mut visibles: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for (_at, row, media) in items {
            let height: usize = match &media {
                Some((_, h)) => *h as usize,
                None => 1,
            };
            if cursor_row >= end {
                cursor_row += height;
                continue;
            }
            if cursor_row >= start {
                let at = pinner.y + (cursor_row - start) as u16;
                match row {
                    preview::DocRow::Text(l) => slice.push(l),
                    preview::DocRow::Media { alt, path } => {
                        let full = resolve_media_path(&path, &dir);
                        let missing = !std::path::Path::new(&full).exists();
                        if let Some((id, _)) = media {
                            if let Some(seq) = preview::media_cell(pv, id, pinner.x, at, cols) {
                                slice.push(Line::from(Span::raw(seq)));
                                visibles.insert(id);
                            } else {
                                slice.push(Line::from(Span::raw(alt)));
                            }
                        } else {
                            slice.push(Line::from(vec![
                                Span::styled("[img ", Style::default().fg(dim).bg(base)),
                                Span::styled(alt, Style::default().fg(textc).bg(base).bold()),
                                Span::styled(
                                    if missing {
                                        format!(" missing: {}]", path)
                                    } else {
                                        format!("]({})", path)
                                    },
                                    Style::default().fg(dim).bg(base),
                                ),
                            ]));
                        }
                        for _ in 1..height.min(view_h) {
                            slice.push(Line::from(""));
                        }
                    }
                }
            }
            cursor_row += height;
        }
        while slice.len() < view_h {
            slice.push(Line::from(""));
        }
        frame.render_widget(Paragraph::new(Text::from(slice)).bg(base), pinner);
        // images scrolled out of view get deleted from a scratch cell
        if !matches!(mode, 401 | 402 | 403 | 12)
            && let Some(seq) = preview::prune_seq(pv, &visibles)
        {
            let x = frame.area().width.saturating_sub(1);
            let y = frame.area().height.saturating_sub(1);
            frame.render_widget(preview::EscapeCell(seq), Rect::new(x, y, 1, 1));
        }
    }

    // bottom: mode pill, live context, position, brand. each fact once.
    let (msg, msg_color): (String, Color) = match mode {
        10 => (
            format!("save: {}", the_command_line),
            hue(theme, "peach").into(),
        ),
        11 => (
            format!("open: {}", the_command_line),
            hue(theme, "peach").into(),
        ),
        401 => ("can't open file".to_string(), errc),
        402 => ("can't save file".to_string(), errc),
        403 => (
            "unsaved work, quit anyway?".to_string(),
            hue(theme, "yellow").into(),
        ),
        2 => {
            let y = (tab.cursor_y as usize).min(tab.input_box.len().saturating_sub(1));
            let len = tab.input_box[y].len();
            let mut a = (tab.cursor_x as usize).min(len);
            let mut b = vis.v_x.min(len);
            while a > 0 && !tab.input_box[y].is_char_boundary(a) {
                a -= 1;
            }
            while b > 0 && !tab.input_box[y].is_char_boundary(b) {
                b -= 1;
            }
            let n = tab.input_box[y][a.min(b)..a.max(b)].chars().count();
            (format!("{} chars selected", n), hue(theme, "mauve").into())
        }
        3 => {
            let n = (tab.cursor_y - vis.v_y as i32).unsigned_abs() as usize + 1;
            (format!("{} lines selected", n), hue(theme, "pink").into())
        }
        _ => {
            let words: usize = tab
                .input_box
                .iter()
                .flat_map(|l| l.split_whitespace())
                .count();
            // detected language, the same engine that colors the buffer
            let lang = highlighter.language(tab);
            (format!("{lang} · {total}L · {words}W"), muted)
        }
    };
    // icons everywhere, rainbow file color, branding bottom right
    let micon = mode_icon(mode);
    let (fgly, _) = file_icon(&tab.file_name);
    let brand = " blur 1.1 ";
    let pos = format!(" {}:{} ", tab.cursor_y + 1, visual_x + 1);

    let pill_label = format!(" {}{} ", micon, label.trim());
    let pill_w = UnicodeWidthStr::width(pill_label.as_str()) + 2;
    let msg_w = UnicodeWidthStr::width(msg.as_str()) + 2;
    let pos_w = UnicodeWidthStr::width(pos.as_str()) + 2;
    let brand_w = UnicodeWidthStr::width(brand) + 2;

    let gap = (status_area.width as usize).saturating_sub(pill_w + msg_w + pos_w + brand_w + 6);

    let status = Line::from(vec![
        // Mode Pill
        Span::styled("", Style::default().fg(mstyle.bg.unwrap_or(base)).bg(base)),
        Span::styled(pill_label, mstyle.bold()),
        Span::styled("", Style::default().fg(mstyle.bg.unwrap_or(base)).bg(base)),
        Span::raw(" "),
        // Message / Info Pill
        Span::styled("", Style::default().fg(fade_rgb(theme, lav, 0.2)).bg(base)),
        Span::styled(
            format!(" {} ", fgly),
            Style::default().fg(textc).bg(fade_rgb(theme, lav, 0.2)),
        ),
        Span::styled(
            msg,
            Style::default().fg(msg_color).bg(fade_rgb(theme, lav, 0.2)),
        ),
        Span::styled("", Style::default().fg(fade_rgb(theme, lav, 0.2)).bg(base)),
        Span::raw(" ".repeat(gap)),
        // Position Pill
        Span::styled("", Style::default().fg(fade_rgb(theme, lav, 0.2)).bg(base)),
        Span::styled(
            pos,
            Style::default()
                .fg(textc)
                .bg(fade_rgb(theme, lav, 0.2))
                .bold(),
        ),
        Span::styled("", Style::default().fg(fade_rgb(theme, lav, 0.2)).bg(base)),
        Span::raw(" "),
        // Brand Pill
        Span::styled("", Style::default().fg(lav).bg(base)),
        Span::styled(
            brand,
            Style::default()
                .fg(fg_color(hue(theme, "mauve")))
                .bg(lav)
                .bold(),
        ),
        Span::styled("", Style::default().fg(lav).bg(base)),
    ]);
    frame.render_widget(Paragraph::new(status).bg(base), status_area);

    // rounded modal for confirm and errors
    lay.modal = None;
    if matches!(mode, 401..=403) {
        let unsaved_n = before.iter().filter(|t| !t.saved).count()
            + (!tab.saved as usize)
            + after.iter().filter(|t| !t.saved).count();
        let all_text = format!("unsaved work in {} tabs, quit anyway?", unsaved_n.max(1));
        let text: &str = match mode {
            401 => "can't open that file",
            402 => "can't save that file",
            _ if quit_all => &all_text,
            _ => "unsaved work, quit anyway?",
        };
        let keys: &str = match mode {
            403 => "[y] quit   [n] stay",
            _ => "[esc] back",
        };
        let w = (UnicodeWidthStr::width(text) + 6)
            .min(frame.area().width.saturating_sub(4) as usize) as u16;
        let h = 4u16.min(frame.area().height.saturating_sub(2));
        let area = Rect::new(
            frame.area().width.saturating_sub(w) / 2,
            frame.area().height / 2 - h / 2,
            w,
            h,
        );
        let pop = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(mcolor))
            .style(Style::default().bg(base));
        let inner = pop.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(pop, area);
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(Span::styled(text, Style::default().fg(textc))),
                Line::from(Span::styled(keys, Style::default().fg(muted))),
            ]))
            .alignment(Alignment::Center),
            inner,
        );
        // left half confirms, right half cancels
        if mode == 403 {
            lay.modal = Some(area);
        }
    }

    // adaptive cursor shape per state. emitted by the event loop, never
    // from inside a frame: a raw stdout write mid-render desynchronises
    // ratatui's own cursor bookkeeping and leaves a second cursor behind.

    // theme picker: quickpick popup with accent dots, current marker,
    // footer hints. geometry is clamped so tiny screens never break.
    lay.picker = None;
    if mode == 12 && !menu.names.is_empty() {
        let fw = frame.area().width;
        let fh = frame.area().height;
        if fw >= 30 && fh >= 10 {
            let n = menu.names.len();
            let rows = (fh as usize)
                .saturating_sub(8)
                .max(3)
                .min(n)
                .min(fh.saturating_sub(4) as usize);
            let name_w = menu
                .display
                .iter()
                .map(|d| UnicodeWidthStr::width(d.as_str()))
                .max()
                .unwrap_or(10);
            let w = ((name_w + 18) as u16).min(fw.saturating_sub(4)).max(28);
            let h = (rows as u16 + 3).min(fh.saturating_sub(2));
            let area = Rect::new(fw.saturating_sub(w) / 2, fh.saturating_sub(h) / 2, w, h);
            let pop = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(mcolor))
                .style(Style::default().bg(base))
                .title_top(Line::from(vec![
                    Span::styled(" ", Style::default().bg(base)),
                    Span::styled("", Style::default().fg(mcolor).bg(base)),
                    Span::styled(
                        format!(" themes · {} ", n),
                        Style::default()
                            .fg(fg_color(hue(theme, "teal")))
                            .bg(mcolor)
                            .bold(),
                    ),
                    Span::styled("", Style::default().fg(mcolor).bg(base)),
                ]));
            let inner_p = pop.inner(area);
            frame.render_widget(Clear, area);
            frame.render_widget(pop, area);
            let top = menu
                .sel
                .saturating_sub(rows.saturating_sub(1) / 2)
                .min(n.saturating_sub(rows));
            lay.picker = Some((area, top, rows));
            let mut lines: Vec<Line> = (0..rows)
                .map(|r| {
                    let i = top + r;
                    let sel_row = i == menu.sel;
                    let current = menu.names.get(i).map(String::as_str) == Some(theme_name);
                    let mut spans = vec![
                        Span::styled(
                            if sel_row { "▌" } else { " " },
                            Style::default()
                                .fg(if sel_row { mcolor } else { faint })
                                .bg(base),
                        ),
                        Span::raw(" "),
                    ];
                    match menu.dots.get(i).copied().flatten() {
                        Some([a, b, c]) => {
                            for (r8, g8, b8) in [a, b, c] {
                                spans.push(Span::styled(
                                    "● ",
                                    Style::default().fg(Color::Rgb(r8, g8, b8)).bg(base),
                                ));
                            }
                        }
                        None => {
                            spans.push(Span::styled("○○○ ", Style::default().fg(faint).bg(base)))
                        }
                    }
                    spans.push(Span::styled(
                        menu.display[i].clone(),
                        if sel_row {
                            Style::default().fg(textc).bg(base).bold()
                        } else {
                            Style::default().fg(muted).bg(base)
                        },
                    ));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        menu.variant[i].clone(),
                        Style::default().fg(faint).bg(base),
                    ));
                    if current {
                        spans.push(Span::styled(" ●", Style::default().fg(mcolor).bg(base)));
                    }
                    Line::from(spans)
                })
                .collect();
            lines.push(Line::from(vec![Span::styled(
                "j/k move · enter apply · esc cancel",
                Style::default().fg(faint).bg(base),
            )]));
            frame.render_widget(Paragraph::new(Text::from(lines)).bg(base), inner_p);
        }
    }

    // one cursor placement per frame, so there is never a second one
    let cursor = if mode == 10 || mode == 11 {
        let prefix = if mode == 10 { "save: " } else { "open: " };
        // cells before typed text:  + pill +  + space + file glyph
        (
            status_area.x
                + 1
                + pill_w as u16
                + 1
                + 1
                + UnicodeWidthStr::width(fgly) as u16
                + UnicodeWidthStr::width(prefix) as u16
                + UnicodeWidthStr::width(the_command_line) as u16,
            status_area.y,
        )
    } else {
        (
            inner.x + gutter_w + visual_x.saturating_sub(tab.scroll_x),
            inner.y + (tab.cursor_y as u16).saturating_sub(tab.scroll_y),
        )
    };
    frame.set_cursor_position(cursor);
}
