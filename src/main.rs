mod controls;
mod helpers;
mod modes;

use helpers::{Highlighter, Tab, Visual, fg_color};
use ratatui::layout::Alignment;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::*;
use unicode_width::UnicodeWidthStr;


fn main() -> std::io::Result<()> {
    ratatui::run(app)?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableBracketedPaste)?;
    let args: Vec<String> = std::env::args().collect();
    let theme = opaline::load_by_name("catppuccin-mocha").unwrap();

    let mut tabs = Vec::new();
    let mut tab_selector: usize = 0;
    let highlighter = Highlighter::new(&theme);
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
            }
            Err(_) => {
                tabs[0].input_box = vec![String::new()];
                tabs[0].file_name = args[1].clone();
            }
        },
    }
    loop {
        let mut tab = &mut tabs[tab_selector];
        terminal.draw(|frame| {
            renderer(
                frame,
                &theme,
                &mut tab,
                &highlighter,
                mode,
                &mut the_command_line,
            )
        })?;

        let event = crossterm::event::read()?;
        let mut the_text = tab.input_box.clone();
        match &event {
            crossterm::event::Event::Paste(text) => match mode {
                1 => modes::insert_paste(&mut tab, text),
                10 | 11 => the_command_line.push_str(text),
                _ => {}
            },
            crossterm::event::Event::Key(event_key) => {
                match mode {
                    0 => {
                        ////////////////////// NORMAL MODE ////////////////////////
                        if !modes::normal_mode(
                            &mut tabs,
                            &mut tab_selector,
                            // &mut tab,
                            &mut vis,
                            *event_key,
                            &mut mode,
                            &mut the_command_line,
                            &mut the_text,
                        )
                        .unwrap()
                        {
                            if tab_selector > 0
                            {
                                tabs.remove(tab_selector);
                                if tab_selector >= tabs.len() - 1
                                {
                                    tab_selector -= 1;
                                }
                                mode = 0;
                            }
                            else {
                                break;
                            }
                        }
                    }
                    1 => {
                        /////////////////////// INSERT MODE /////////////////////////
                        if !modes::insert_mode(&mut tab, *event_key, &mut mode, &mut the_text, &mut filled_now)
                            .unwrap()
                        {
                            continue;
                        }
                    }
                    2 => {
                        //////////////////////// SELECT MODE ////////////////////////////////////
                        if !modes::select_mode1(&mut tab, &mut vis, *event_key, &mut mode).unwrap()
                        {
                            mode = 0;
                            continue;
                        }
                    }
                    3 => {
                        //////////////////////// SELECT-LINE MODE ////////////////////////////////////
                        if !modes::select_mode_line(&mut tab, &mut vis, *event_key, &mut mode).unwrap()
                        {
                            mode = 0;
                            continue;
                        }
                    }
                    ////////////////////// SAVE/OPEN MODES ////////////////////////////////
                    10 => {
                        if !modes::save_mode(&mut tab, *event_key, &mut the_command_line, &mut mode)
                            .unwrap()
                        {
                            mode = 402;
                        }
                    }
                    11 => {
                        if !modes::open_mode(&mut tab, *event_key, &mut the_command_line, &mut mode)
                            .unwrap()
                        {
                            mode = 401;
                        }
                    }
                    ////////////////////// UNSAVED WORK MODE ////////////////////////////////
                    403 => {
                        if !modes::unsaved_work_mode(*event_key, &mut mode).unwrap() {
                            if tab_selector > 0
                            {
                                tabs.remove(tab_selector);
                                if tab_selector >= tabs.len() - 1
                                {
                                    tab_selector -= 1;
                                }
                                mode = 0;
                            }
                            else {
                                break;
                            }
                        }
                    }
                    _ => {
                        if crossterm::event::read()?.is_key_press() {
                            the_command_line.clear();
                            mode = 0;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    crossterm::execute!(std::io::stdout(), crossterm::event::DisableBracketedPaste)?;
    Ok(())
}

fn renderer(
    frame: &mut Frame,
    theme: &opaline::Theme,
    tab: &mut Tab,
    highlighter: &Highlighter,
    mode: i32,
    the_command_line: &str,
) {
    let areas = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Min(0),
        ratatui::layout::Constraint::Length(1),
    ])
    .split(frame.area());

    let footer_text: String;
    let footer_chunks =
        ratatui::layout::Layout::horizontal([ratatui::layout::Constraint::Percentage(50); 2])
            .split(areas[1]);

    match mode {
        0 => {
            footer_text = format!(" NORMAL ");
        }
        1 => {
            footer_text = format!(" INSERT ");
        }
        2 => {
            footer_text = format!(" SELECT ");
        }
        3 => {
            footer_text = format!(" SELECT-LINE ");
        }
        10 => {
            footer_text = format!(" Save file into: {} ", the_command_line);
        }
        11 => {
            footer_text = format!(" File to Open: {} ", the_command_line);
        }
        401 => {
            footer_text = format!(" Can't open file ");
        }
        402 => {
            footer_text = format!(" Can't save file ");
        }
        403 => footer_text = format!("you have unsaved work, quit anyway? [y/n]"),
        _ => {
            footer_text =
                "SOME ERRORS, try to relaunch the program                   BLUR V0.1".to_string();
        }
    }

    if tab.cursor_y as u16 <= tab.scroll_y {
        tab.scroll_y = tab.cursor_y as u16;
    } else if tab.cursor_y as u16 >= tab.scroll_y + areas[0].height {
        tab.scroll_y = tab.cursor_y as u16 - areas[0].height + 1;
    }

    let empty = String::new();
    let current_line = tab
        .input_box
        .iter()
        .nth(tab.cursor_y as usize)
        .unwrap_or(&empty);
    let visual_x = current_line
        .get(..tab.cursor_x as usize)
        .map(UnicodeWidthStr::width)
        .unwrap_or(tab.cursor_x as usize) as u16;

    if visual_x <= tab.scroll_x {
        tab.scroll_x = visual_x;
    } else if visual_x >= tab.scroll_x + areas[0].width {
        tab.scroll_x = visual_x - areas[0].width + 1;
    }

    let footer_file_name = format!(
        " {} ",
        if tab.file_name.is_empty() {
            "[Empty File]*".to_string()
        } else {
            if tab.saved {
                tab.file_name.clone()
            } else {
                format!("*{}", tab.file_name.clone())
            }
        }
    );
    let footer_left = Line::from(vec![
        Span::styled(
            footer_text.clone(),
            Style::default()
                .fg(fg_color(theme.color("accent.primary")))
                .bg(theme.color("accent.primary").into())
                .bold(),
        ),
        Span::styled(
            "\u{e0b0}",
            Style::default()
                .fg(theme.color("accent.primary").into())
                .bg(theme.color("accent.secondary").into()),
        ),
        Span::styled(
            footer_file_name,
            Style::default()
                .fg(fg_color(theme.color("accent.secondary")))
                .bg(theme.color("accent.secondary").into())
                .bold(),
        ),
        Span::styled(
            "\u{e0b0}",
            Style::default().fg(theme.color("accent.secondary").into()),
        ),
    ]);
    let footer_right = Line::from(vec![
        Span::raw(format!(" Row {}; Col {} ", visual_x, tab.cursor_y)),
        Span::styled(
            "\u{e0b2}",
            Style::default().fg(theme.color("accent.primary").into()),
        ),
        Span::styled(
            " Blur 0.1.1 ",
            Style::default()
                .fg(fg_color(theme.color("accent.primary")))
                .bg(theme.color("accent.primary").into())
                .bold(),
        ),
    ]);

    let input =
        ratatui::widgets::Paragraph::new(ratatui::text::Text::from(highlighter.highlight(tab)))
            .scroll((tab.scroll_y, tab.scroll_x));
    frame.render_widget(input, areas[0]);
    frame.render_widget(
        ratatui::widgets::Paragraph::new(footer_left)
            .alignment(Alignment::Left)
            .bg::<ratatui::style::Color>(theme.color("bg.base").into()),
        footer_chunks[0],
    );
    frame.render_widget(
        ratatui::widgets::Paragraph::new(footer_right)
            .alignment(Alignment::Right)
            .bg::<ratatui::style::Color>(theme.color("bg.base").into()),
        footer_chunks[1],
    );

    frame.set_cursor_position((
        areas[0].x + visual_x.saturating_sub(tab.scroll_x),
        areas[0].y + (tab.cursor_y as u16).saturating_sub(tab.scroll_y),
    ));
    if mode == 10 || mode == 11 {
        let prefix = if mode == 10 {
            " Save file into: "
        } else {
            " File to Open: "
        };
        let cursor_col = prefix.chars().count() + the_command_line.chars().count();
        frame.set_cursor_position((areas[1].x + cursor_col as u16, areas[1].y));
    }
}
