mod controls;
mod helpers;
mod modes;
mod normal_mode;
mod select_modes;

use helpers::{Highlighter, Tab, Visual, fg_color};
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

fn apply_theme(
    theme: &mut opaline::Theme,
    highlighter: &mut Highlighter,
    tabs: &mut [Tab],
    name: &str,
) {
    if let Some(t) = opaline::load_by_name(name) {
        *theme = t;
        highlighter.set_theme(theme);
        for tab in tabs.iter_mut() {
            tab.highlight_cache = None;
        }
    }
}

fn fill_dots(menu: &mut ThemeMenu) {
    for (i, name) in menu.names.iter().enumerate() {
        if menu.dots[i].is_some() {
            continue;
        }
        menu.dots[i] = opaline::load_by_name(name).and_then(|t| {
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

fn load_config_theme() -> Option<String> {
    let text = std::fs::read_to_string(config_path()?).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix("theme") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let Some(name) = rest
            .trim()
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
        else {
            continue;
        };
        if opaline::load_by_name(name).is_some() {
            return Some(name.to_string());
        }
    }
    None
}

fn save_config_theme(name: &str) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, format!("theme = \"{}\"\n", name));
    }
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste)?;
    let args: Vec<String> = std::env::args().collect();
    let mut theme_name = load_config_theme().unwrap_or_else(|| String::from("catppuccin-mocha"));
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
            crossterm::event::Event::Key(event_key) => {
                match mode {
                    0 => {
                        // theme picker, kept here so normal_mode stays edit-only
                        if event_key.code == crossterm::event::KeyCode::Char('t') {
                            menu.sel = menu
                                .names
                                .iter()
                                .position(|n| n == &theme_name)
                                .unwrap_or(0);
                            theme_prev = theme_name.clone();
                            fill_dots(&mut menu);
                            mode = 12;
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
                            if tabs.len() > 1 {
                                tabs.remove(tab_selector);
                                if tab_selector >= tabs.len() {
                                    tab_selector = tabs.len() - 1;
                                }
                                mode = 0;
                            } else {
                                break;
                            }
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
                        if !modes::open_mode(tab, *event_key, &mut the_command_line, &mut mode)
                            .unwrap()
                        {
                            mode = 401;
                        }
                    }

                    403 => {
                        if !modes::unsaved_work_mode(*event_key, &mut mode).unwrap() {
                            if tabs.len() > 1 {
                                tabs.remove(tab_selector);
                                if tab_selector >= tabs.len() {
                                    tab_selector = tabs.len() - 1;
                                }
                                mode = 0;
                            } else {
                                break;
                            }
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
                                if let Some(name) = menu.names.get(menu.sel) {
                                    theme_name = name.clone();
                                    save_config_theme(&theme_name);
                                }
                                mode = 0;
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
    crossterm::execute!(
        std::io::stdout(),
        crossterm::event::DisableBracketedPaste,
        crossterm::cursor::SetCursorStyle::DefaultUserShape,
    )?;
    Ok(())
}

/// resolve a hue across every opaline theme. only the catppuccin
/// family defines raw names like "blue" or "mauve", so each hue falls
/// back through shared semantic tokens. never returns FALLBACK.
fn hue(theme: &opaline::Theme, hue: &str) -> opaline::OpalineColor {
    if let Some(c) = theme.try_color(hue) {
        return c;
    }
    let fallbacks: &[&str] = match hue {
        "blue" => &["accent.secondary", "info"],
        "green" => &["success", "accent.primary"],
        "mauve" => &["accent.primary", "accent.tertiary"],
        "pink" => &["accent.tertiary", "accent.primary"],
        "peach" => &["warning", "accent.tertiary"],
        "red" => &["error"],
        "yellow" => &["warning"],
        "teal" => &["accent.secondary", "success"],
        "sky" | "sapphire" => &["accent.secondary", "info"],
        "lavender" => &["accent.primary"],
        "overlay0" | "subtext0" => &["text.dim"],
        _ => &[],
    };
    fallbacks
        .iter()
        .find_map(|k| theme.try_color(k))
        .unwrap_or_else(|| theme.color("text.primary"))
}

/// resolve a structural token with a safe fallback chain.
fn tok(theme: &opaline::Theme, key: &str) -> opaline::OpalineColor {
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
        0 => "i insert · v visual · t themes · w save · q quit",
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

    // scroll, cursor rests near top third (inside rounded box)
    let edit_h = edit_area.height.saturating_sub(2);
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
    // gutter renders as `{num:>digits} `, exactly digits + 1 cells
    let gutter_w = digits + 1;
    let text_w = edit_area.width.saturating_sub(gutter_w + 4);
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
    let pills: Vec<(Vec<Span>, usize)> = all
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let active = i == active_idx;
            let name = shorten_middle(&names[i], 24);
            let (glyph, _) = file_icon(&t.file_name);
            let mut spans = Vec::new();
            if active {
                spans.push(Span::styled("", Style::default().fg(mcolor).bg(base)));
                spans.push(Span::styled(
                    format!(" {} ", i + 1),
                    Style::default()
                        .fg(fg_color(hue(theme, mode_key)))
                        .bg(mcolor)
                        .bold(),
                ));
                spans.push(Span::styled(
                    glyph,
                    Style::default()
                        .fg(fg_color(hue(theme, mode_key)))
                        .bg(mcolor),
                ));
                spans.push(Span::styled(
                    format!("{} ", name),
                    Style::default()
                        .fg(fg_color(hue(theme, mode_key)))
                        .bg(mcolor)
                        .bold(),
                ));
                if !t.saved {
                    spans.push(Span::styled("● ", Style::default().fg(errc).bg(mcolor)));
                }
                spans.push(Span::styled("", Style::default().fg(mcolor).bg(base)));
            } else {
                spans.push(Span::styled(
                    format!(" {} ", i + 1),
                    Style::default().fg(dim).bg(base),
                ));
                spans.push(Span::styled(glyph, Style::default().fg(textc).bg(base)));
                spans.push(Span::styled(name, Style::default().fg(dim).bg(base)));
                if !t.saved {
                    spans.push(Span::styled(" ●", Style::default().fg(errc).bg(base)));
                }
            }
            spans.push(Span::styled(" ", Style::default().bg(base)));
            let w: usize = spans
                .iter()
                .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                .sum();
            (spans, w)
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
    if lo > 0 {
        tabline
            .spans
            .push(Span::styled("‹ ", Style::default().fg(dim).bg(base)));
    }
    for (spans, _) in pills.iter().take(hi_excl).skip(lo) {
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
    let mut visible: Vec<Line> = highlighter
        .highlight(tab)
        .into_iter()
        .enumerate()
        .skip(tab.scroll_y as usize)
        .take(edit_h as usize)
        .map(|(i, mut line)| {
            let cur = i as i32 == tab.cursor_y;
            let selected = mode == 3
                && vis.on
                && (i as i32 - vis.v_y as i32).abs() + (vis.v_y as i32 - tab.cursor_y).abs()
                    == (i as i32 - tab.cursor_y).abs();

            let num = format!("{:>w$} ", i + 1, w = digits as usize);
            let num_st = if cur {
                Style::default().fg(lav).bold()
            } else if selected {
                Style::default().fg(textc).bold()
            } else {
                Style::default().fg(faint)
            };
            let mut spans = vec![Span::styled(num, num_st)];
            if cur {
                for s in &mut line.spans {
                    s.style = s.style.add_modifier(Modifier::UNDERLINED);
                }
            } else if selected {
                for s in &mut line.spans {
                    s.style = s.style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
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
        // title shows the parent dir, the name already lives in the tabline
        .title_top({
            let dir = tab
                .file_name
                .rsplit_once('/')
                .map(|(d, _)| d.to_string())
                .unwrap_or_else(|| "untitled".to_string());
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
    let inner = block.inner(edit_area);
    frame.render_widget(block, edit_area);

    let code_w = inner.width.saturating_sub(1);
    if total > 1 && total as u16 > inner.height && code_w > 0 && inner.height > 0 {
        let h = inner.height as usize;
        let pos = (tab.cursor_y as usize * h.saturating_sub(1) / total.saturating_sub(1))
            .min(h.saturating_sub(1));
        for row in 0..h {
            let active = row == pos;
            let ch = if active { "█" } else { "·" };
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

    // bottom: mode pill, live context, position, brand. each fact once.
    let msg: String = match mode {
        10 => format!("save: {}", the_command_line),
        11 => format!("open: {}", the_command_line),
        401 => "can't open".to_string(),
        402 => "can't save".to_string(),
        403 => "unsaved work, quit anyway?".to_string(),
        2 => {
            let y = (tab.cursor_y as usize).min(tab.input_box.len().saturating_sub(1));
            let len = tab.input_box[y].len();
            let mut a = (tab.cursor_x as usize).min(len);
            let mut b = vis.v_x.min(len);
            while !tab.input_box[y].is_char_boundary(a) {
                a -= 1;
            }
            while !tab.input_box[y].is_char_boundary(b) {
                b -= 1;
            }
            let n = tab.input_box[y][a.min(b)..a.max(b)].chars().count();
            format!("{} chars selected", n)
        }
        3 => {
            let n = (tab.cursor_y - vis.v_y as i32).unsigned_abs() as usize + 1;
            format!("{} lines selected", n)
        }
        _ => {
            let words: usize = tab
                .input_box
                .iter()
                .flat_map(|l| l.split_whitespace())
                .count();
            format!("{} lines · {} words · {}", total, words, theme_name)
        }
    };
    // icons everywhere, rainbow file color, branding bottom right
    let micon = mode_icon(mode);
    let (fgly, _) = file_icon(&tab.file_name);
    let mauve: Color = hue(theme, "mauve").into();
    let brand = " blur 0.10 ";
    let pos = format!("{}:{}", tab.cursor_y + 1, visual_x + 1);
    let pill_label = format!("{}{}", micon, label.trim());
    let pill_w = UnicodeWidthStr::width(pill_label.as_str()) + 2;
    // file glyph carries its own trailing space, msg starts right after it
    let msg_w = UnicodeWidthStr::width(msg.as_str()) + 2;
    let pos_w = UnicodeWidthStr::width(pos.as_str());
    let brand_w = UnicodeWidthStr::width(brand) + 2;
    // cells: pill(+content+) + space + glyph + msg + gap + pos + space + brand(+content+)
    let gap = (status_area.width as usize).saturating_sub(pill_w + msg_w + pos_w + brand_w + 5);
    let status = Line::from(vec![
        Span::styled("", Style::default().fg(mstyle.bg.unwrap_or(base)).bg(base)),
        Span::styled(format!(" {}{} ", micon, label.trim()), mstyle.bold()),
        Span::styled("", Style::default().fg(mstyle.bg.unwrap_or(base)).bg(base)),
        Span::raw(" "),
        Span::styled(fgly, Style::default().fg(textc).bg(base)),
        Span::styled(msg.clone(), Style::default().fg(muted).bg(base)),
        Span::raw(" ".repeat(gap)),
        Span::styled(format!(" {}", pos), Style::default().fg(muted).bg(base)),
        Span::raw(" "),
        Span::styled("", Style::default().fg(mauve).bg(base)),
        Span::styled(
            brand,
            Style::default()
                .fg(fg_color(hue(theme, "mauve")))
                .bg(mauve)
                .bold(),
        ),
        Span::styled("", Style::default().fg(mauve).bg(base)),
    ]);
    frame.render_widget(Paragraph::new(status).bg(base), status_area);

    // rounded modal for confirm and errors
    if matches!(mode, 401..=403) {
        let text: &str = match mode {
            401 => "can't open that file",
            402 => "can't save that file",
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
    }

    // adaptive cursor shape per state: block at rest, bar while typing,
    // underline while selecting. steady so it stays sharp, blinking bar
    // in insert so the typing point reads instantly.
    let shape = match mode {
        1 | 10 | 11 => crossterm::cursor::SetCursorStyle::BlinkingBar,
        2 | 3 => crossterm::cursor::SetCursorStyle::SteadyUnderScore,
        _ => crossterm::cursor::SetCursorStyle::SteadyBlock,
    };
    let _ = crossterm::execute!(std::io::stdout(), shape);

    // theme picker: centered scrollable list, three accent dots per row
    // theme picker: quickpick popup with accent dots, current marker,
    // footer hints. geometry is clamped so tiny screens never break.
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

    // cursor inside the rounded box
    frame.set_cursor_position((
        inner.x + gutter_w + visual_x.saturating_sub(tab.scroll_x),
        inner.y + (tab.cursor_y as u16).saturating_sub(tab.scroll_y),
    ));
    if mode == 10 || mode == 11 {
        let prefix = if mode == 10 { "save: " } else { "open: " };
        // cells before typed text:  + pill +  + space + file glyph
        let x = status_area.x
            + 1
            + pill_w as u16
            + 1
            + 1
            + UnicodeWidthStr::width(fgly) as u16
            + UnicodeWidthStr::width(prefix) as u16
            + UnicodeWidthStr::width(the_command_line) as u16;
        frame.set_cursor_position((x, status_area.y));
    }
}
