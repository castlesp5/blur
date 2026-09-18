use crate::controls::{controls, default_controls};
use crate::helpers::{EditRecord, Tab, Visual, apply_forward, apply_inverse};

pub fn normal_mode(
    tabs: &mut Vec<Tab>,
    tab_selector: &mut usize,
    // tab: &mut Tab,
    vis: &mut Visual,
    event_key: crossterm::event::KeyEvent,
    mode: &mut i32,
    the_command_line: &mut String,
    text: &mut Vec<String>,
) -> std::io::Result<bool> {
    let mut tab = &mut tabs[*tab_selector];
    if controls(event_key, &mut tab.cursor_y, &mut tab.cursor_x, text).unwrap() {
        return Ok(true);
    }
    match event_key.code {
        crossterm::event::KeyCode::Char('N') => {
            tabs.push(Tab::new());
            *tab_selector += 1;
        }
        crossterm::event::KeyCode::Char('q') => {
            if !tab.saved {
                *mode = 403;
            } else {
                return Ok(false);
            }
        }
        crossterm::event::KeyCode::Tab | crossterm::event::KeyCode::Char('n') => {
            if *tab_selector < tabs.len() - 1
            {
                *tab_selector += 1;
            }
            else 
            {
                *tab_selector = 0;
            }
        }
        crossterm::event::KeyCode::BackTab => {
            if *tab_selector > 0
            {
                *tab_selector -= 1;
            }
            else 
            {
                *tab_selector = tabs.len() - 1;
            }
        }
        crossterm::event::KeyCode::Char('a') => {
            if (tab.cursor_x as usize) < text[tab.cursor_y as usize].len() {
                tab.cursor_x += 1;
            }
            *mode = 1;
        }
        crossterm::event::KeyCode::Char('i') => {
            *mode = 1;
        }
        crossterm::event::KeyCode::Char('K') => {
            if tab.cursor_y as usize > 0
            {
                let text = tab.input_box.remove(tab.cursor_y as usize);
                tab.saved = false;
                tab.input_box.insert(tab.cursor_y as usize - 1, text);
                tab.cursor_y -= 1
            }
        }
        crossterm::event::KeyCode::Char('J') => {
            if (tab.cursor_y as usize) < tab.input_box.len() - 1
            {
                let text = tab.input_box.remove(tab.cursor_y as usize);
                tab.saved = false;
                tab.input_box.insert(tab.cursor_y as usize + 1, text);
                tab.cursor_y += 1;
            }
        }
        crossterm::event::KeyCode::Char('>') => {
            tab.saved = false;
            tab.input_box[tab.cursor_y as usize].insert_str(0, "    ");
        }
        crossterm::event::KeyCode::Char('<') => {
            tab.saved = false;
            if &tab.input_box[tab.cursor_y as usize][0..4] == "    "
            {
                tab.input_box[tab.cursor_y as usize].replace_range(0..4, "");
            }
        }
        crossterm::event::KeyCode::Char('e') => {
            let start = tab.cursor_x as usize;
            let new_x = match text[tab.cursor_y as usize][start..].find(' ') {
                Some(rel) => start + rel + 1,
                None => text[tab.cursor_y as usize].len(),
            } as i32;

            tab.cursor_x = new_x;
        }
        crossterm::event::KeyCode::Char('b') => {
            let start = tab.cursor_x as usize;
            let before = &text[tab.cursor_y as usize][..start];
            let new_x = match before.rfind(' ') {
                Some(rel) => rel,
                None => 0,
            } as i32;

            tab.cursor_x = new_x;
        }
        crossterm::event::KeyCode::Char('o') => {
            tab.saved = false;
            tab.input_box
                .insert(tab.cursor_y as usize + 1, String::new());
            tab.undo_stack.push(EditRecord::InsertLine {
                row: tab.cursor_y as usize + 1,
            });
            tab.redo_stack.clear();
            tab.cursor_y += 1;
            tab.cursor_x = 0;
            *mode = 1;
        }
        crossterm::event::KeyCode::Char('w') => {
            if tab.file_name.len() > 0 {
                match std::fs::write(&tab.file_name, &tab.input_box.join("\n")) {
                    Ok(_) => {
                        tab.saved = true;
                    }
                    Err(_) => {
                        *mode = 402;
                    }
                }
                *mode = 0;
                the_command_line.clear();
                return Ok(true);
            }
            *mode = 10;
        }
        crossterm::event::KeyCode::Char('W') => {
            *mode = 10;
        }
        crossterm::event::KeyCode::Char('O') => {
            *mode = 11;
        }
        crossterm::event::KeyCode::Delete => {
            let x = tab.cursor_x as usize;
            let y = tab.cursor_y as usize;
            tab.saved = false;
            if tab.input_box[y].is_empty() && tab.input_box.len() > 1 {
                tab.input_box.remove(y);
                tab.cursor_x = 0;
                tab.undo_stack.push(EditRecord::RemoveEmptyLine { row: y });
                tab.redo_stack.clear();
            } else if x < tab.input_box[y].len() {
                let ch = tab.input_box[y].remove(x);
                tab.undo_stack
                    .push(EditRecord::DeleteChar { row: y, col: x, ch });
                tab.redo_stack.clear();
            }
        }
        crossterm::event::KeyCode::Backspace => {
            let y = tab.cursor_y as usize;
            tab.saved = false;
            if tab.input_box[y].is_empty() && y > 0 {
                tab.input_box.remove(y);
                tab.undo_stack.push(EditRecord::RemoveEmptyLine { row: y });
                tab.cursor_y -= 1;
                tab.redo_stack.clear();
                tab.cursor_x = text[tab.cursor_y as usize].len() as i32;
                return Ok(true);
            } else if tab.cursor_x > 0 {
                let x = tab.cursor_x as usize;
                if let Some((prev_idx, ch)) = tab.input_box[y][..x].char_indices().next_back() {
                    tab.input_box[y].remove(prev_idx);
                    tab.undo_stack.push(EditRecord::DeleteChar {
                        row: y,
                        col: prev_idx,
                        ch,
                    });
                    tab.cursor_x -= ch.len_utf8() as i32;
                    tab.redo_stack.clear();
                }
            } else {
                if y > 0 {
                    let current = tab.input_box.remove(y);
                    let prev_line = &mut tab.input_box[y - 1];
                    tab.cursor_y -= 1;
                    tab.cursor_x = prev_line.len() as i32;
                    prev_line.push_str(&current);
                    tab.undo_stack.push(EditRecord::MergeLine {
                        row: y - 1,
                        prev_len: tab.cursor_x as usize,
                    });
                    tab.redo_stack.clear();
                }
            }
        }
        crossterm::event::KeyCode::Char('d') => {
            let y = tab.cursor_y as usize;
            let line = tab.input_box.remove(y);
            tab.cursor_x = 0;
            if tab.input_box.len() == 0 {
                tab.input_box.push(String::new());
            }

            tab.undo_stack.push(EditRecord::RemoveLine {
                row: y,
                content: line.clone(),
            });
            tab.redo_stack.clear();

        }
        crossterm::event::KeyCode::Char('u') => {
            if let Some(record) = tab.undo_stack.pop() {
                let (row, col) = apply_inverse(&record, &mut tab.input_box);
                tab.saved = false;
                tab.cursor_y = row as i32;
                tab.cursor_x = col as i32;
                tab.redo_stack.push(record);
            }
        }
        crossterm::event::KeyCode::Char('r') => {
            if let Some(record) = tab.redo_stack.pop() {
                let (row, col) = apply_forward(&record, &mut tab.input_box);
                tab.saved = false;
                tab.cursor_y = row as i32;
                tab.cursor_x = col as i32;
                tab.undo_stack.push(record);
            }
        }
        crossterm::event::KeyCode::Char('v') => {
            *mode = 2;
            vis.v_x = tab.cursor_x as usize;
            vis.v_y = tab.cursor_y as usize;
            vis.on = true;
        }
        crossterm::event::KeyCode::Char('V') => {
            *mode = 3;
            vis.v_x = tab.cursor_x as usize;
            vis.v_y = tab.cursor_y as usize;
            vis.on = true;
        }
        _ => {}
    }
    Ok(true)
}


pub fn select_mode_line(
    tab: &mut Tab,
    vis: &mut Visual,
    event_key: crossterm::event::KeyEvent,
    mode: &mut i32,
) -> std::io::Result<bool>
{
    if controls(event_key, &mut tab.cursor_y, &mut tab.cursor_x, &mut tab.input_box)? {
        return Ok(true);
    }
    match event_key.code {
        crossterm::event::KeyCode::Esc => {
            *mode = 0;
            return Ok(false);
        }
        crossterm::event::KeyCode::Char('d') => {
            // let line = &mut tab.input_box[tab.cursor_y as usize];

            tab.saved = false;
            if tab.cursor_y as usize > vis.v_y {
                for _ in vis.v_y..tab.cursor_y as usize + 1
                {
                    tab.undo_stack.push(EditRecord::RemoveLine {
                        row: vis.v_y,
                        content: tab.input_box[vis.v_y].to_string(),
                    });
                    tab.redo_stack.clear();
                    tab.input_box.remove(vis.v_y);
                }
                tab.cursor_y = vis.v_y as i32;
            }
            else if vis.v_y > tab.cursor_y as usize {
                for _ in tab.cursor_y as usize..vis.v_y + 1
                {
                    tab.undo_stack.push(EditRecord::RemoveLine {
                        row: tab.cursor_y as usize,
                        content: tab.input_box[tab.cursor_y as usize].to_string(),
                    });
                    tab.redo_stack.clear();
                    tab.input_box.remove(tab.cursor_y as usize);
                }
            }
        }
        _ => {*mode = 0}
    }
    Ok(false)
}

pub fn select_mode1(
    tab: &mut Tab,
    vis: &mut Visual,
    event_key: crossterm::event::KeyEvent,
    mode: &mut i32,
) -> std::io::Result<bool>
{
    if controls(event_key, &mut tab.cursor_y, &mut tab.cursor_x, &mut tab.input_box)? {
        return Ok(true);
    }
    match event_key.code {
        crossterm::event::KeyCode::Esc => {
            *mode = 0;
            return Ok(false);
        }
        crossterm::event::KeyCode::Char('d') => {
            if tab.cursor_y as usize == vis.v_y
            {
                let line = &mut tab.input_box[tab.cursor_y as usize];

                tab.saved = false;
                if tab.cursor_x as usize > vis.v_x {
                    tab.undo_stack.push(EditRecord::RemoveString {
                        row: tab.cursor_y as usize,
                        col: vis.v_x,
                        text: line[vis.v_x..tab.cursor_x as usize].to_string(),
                    });
                    tab.redo_stack.clear();

                    line.drain(vis.v_x..tab.cursor_x as usize);

                    tab.cursor_x = vis.v_x as i32;
                }
                else if vis.v_x > tab.cursor_x as usize {
                    tab.undo_stack.push(EditRecord::RemoveString {
                        row: tab.cursor_y as usize,
                        col: tab.cursor_x as usize,
                        text: line[tab.cursor_x as usize..vis.v_x].to_string(),
                    });
                    tab.redo_stack.clear();
                    line.drain(tab.cursor_x as usize..vis.v_x);
                }
            }
        }
        _ => {*mode = 0}
    }
    Ok(false)
}


pub fn insert_mode(
    tab: &mut Tab,
    event_key: crossterm::event::KeyEvent,
    mode: &mut i32,
    text: &mut Vec<String>,
    filled_now: &mut String,
) -> std::io::Result<bool> {
    if !matches!(event_key.code, crossterm::event::KeyCode::Char(_)) {
        if !filled_now.is_empty() {
            let col = tab.cursor_x as usize - filled_now.len();
            tab.undo_stack.push(EditRecord::InsertString {
                row: tab.cursor_y as usize,
                col,
                text: filled_now.clone(),
            });
            tab.redo_stack.clear();
        }
        filled_now.clear();
    }
    if default_controls(event_key, &mut tab.cursor_y, &mut tab.cursor_x, text)? {
        return Ok(true);
    }
    match event_key.code {
        crossterm::event::KeyCode::Esc => {
            *mode = 0;
            return Ok(false);
        }

        crossterm::event::KeyCode::Char(c) => {
            let blen = c.len_utf8() as i32;
            tab.saved = false;
            let row = tab.cursor_y as usize;
            let col = tab.cursor_x as usize;
            tab.input_box[row].insert(col, c);
            filled_now.push(c);
            tab.redo_stack.clear();
            tab.cursor_x += blen;
        }

        crossterm::event::KeyCode::Enter => {
            let y = tab.cursor_y as usize;
            let x = tab.cursor_x as usize;
            let rest = tab.input_box[y].split_off(x);
            tab.input_box.insert(y + 1, rest);
            tab.undo_stack
                .push(EditRecord::SplitLine { row: y, col: x });
            tab.redo_stack.clear();
            tab.cursor_x = 0;
            tab.cursor_y += 1;
        }
        crossterm::event::KeyCode::Tab => {
            tab.saved = false;
            let row = tab.cursor_y as usize;
            let col = tab.cursor_x as usize;
            tab.input_box[row].insert_str(col, "    ");
            tab.undo_stack.push(EditRecord::InsertString {
                row,
                col,
                text: "    ".to_string(),
            });
            tab.redo_stack.clear();
            tab.cursor_x += 4;
        }
        crossterm::event::KeyCode::Delete => {
            let x = tab.cursor_x as usize;
            let y = tab.cursor_y as usize;
            tab.saved = false;
            if tab.input_box[y].is_empty() && tab.input_box.len() > 1 {
                tab.input_box.remove(y);
                tab.cursor_x = 0;
                tab.undo_stack.push(EditRecord::RemoveEmptyLine { row: y });
                tab.redo_stack.clear();
            } else if x < tab.input_box[y].len() {
                let ch = tab.input_box[y].remove(x);
                tab.undo_stack
                    .push(EditRecord::DeleteChar { row: y, col: x, ch });
                tab.redo_stack.clear();
            }
        }
        crossterm::event::KeyCode::Backspace => {
            let y = tab.cursor_y as usize;
            tab.saved = false;
            if tab.input_box[y].is_empty() && y > 0 {
                tab.input_box.remove(y);
                tab.undo_stack.push(EditRecord::RemoveEmptyLine { row: y });
                tab.cursor_y -= 1;
                tab.redo_stack.clear();
                tab.cursor_x = text[tab.cursor_y as usize].len() as i32;
                return Ok(true);
            }
            if tab.cursor_x > 0 {
                let x = tab.cursor_x as usize;
                if let Some((prev_idx, ch)) = tab.input_box[y][..x].char_indices().next_back() {
                    tab.input_box[y].remove(prev_idx);
                    tab.undo_stack.push(EditRecord::DeleteChar {
                        row: y,
                        col: prev_idx,
                        ch,
                    });
                    tab.redo_stack.clear();
                    tab.cursor_x -= ch.len_utf8() as i32;
                }
            } else {
                if y > 0 {
                    let current = tab.input_box.remove(y);
                    let prev_line = &mut tab.input_box[y - 1];
                    tab.cursor_y -= 1;
                    tab.cursor_x = prev_line.len() as i32;
                    prev_line.push_str(&current);
                    tab.undo_stack.push(EditRecord::MergeLine {
                        row: y - 1,
                        prev_len: tab.cursor_x as usize,
                    });
                    tab.redo_stack.clear();
                }
            }
        }
        _ => {}
    }
    Ok(true)
}

pub fn unsaved_work_mode(
    event_key: crossterm::event::KeyEvent,
    mode: &mut i32,
) -> std::io::Result<bool> {
    match event_key.code {
        crossterm::event::KeyCode::Char('y') => {
            return Ok(false);
        }
        crossterm::event::KeyCode::Char('n') => {
            *mode = 0;
        }
        _ => {}
    }
    Ok(true)
}

pub fn insert_paste(tab: &mut Tab, text: &str) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");

    tab.saved = false;

    let y = tab.cursor_y as usize;
    let x = tab.cursor_x as usize;

    let mut lines = text.split('\n');

    let before = tab.input_box[y][..x].to_string();
    let after = tab.input_box[y][x..].to_string();

    let first = lines.next().unwrap_or("");

    tab.input_box[y] = format!("{}{}", before, first);

    let mut new_y = y;

    for line in lines {
        new_y += 1;
        tab.input_box.insert(new_y, line.to_string());
    }

    tab.input_box[new_y].push_str(&after);
    tab.cursor_y = new_y as i32;
    tab.cursor_x = tab.input_box[new_y].len() as i32 - after.len() as i32;
}

pub fn open_mode(
    tab: &mut Tab,
    event_key: crossterm::event::KeyEvent,
    the_command_line: &mut String,
    mode: &mut i32,
) -> std::io::Result<bool> {
    match event_key.code {
        crossterm::event::KeyCode::Char(c) => {
            the_command_line.push(c);
        }
        crossterm::event::KeyCode::Backspace => {
            the_command_line.pop();
        }
        crossterm::event::KeyCode::Enter => {
            match std::fs::read_to_string(&the_command_line) {
                Ok(content) => {
                    tab.undo_stack.clear();
                    tab.redo_stack.clear();
                    tab.input_box = content.clone().split('\n').map(|s| s.to_string()).collect()
                }
                Err(_) => {
                    tab.input_box = vec![String::new()];
                }
            }
            tab.cursor_x = 0;
            tab.cursor_y = 0;
            tab.file_name = the_command_line.clone();
            the_command_line.clear();
            *mode = 0;
        }
        crossterm::event::KeyCode::Esc => {
            the_command_line.clear();
            *mode = 0;
        }
        _ => {}
    }
    Ok(true)
}
pub fn save_mode(
    tab: &mut Tab,
    event_key: crossterm::event::KeyEvent,
    the_command_line: &mut String,
    mode: &mut i32,
) -> std::io::Result<bool> {
    match event_key.code {
        crossterm::event::KeyCode::Char(c) => {
            the_command_line.push(c);
        }
        crossterm::event::KeyCode::Backspace => {
            the_command_line.pop();
        }
        crossterm::event::KeyCode::Enter => {
            tab.file_name = the_command_line.to_string();
            match std::fs::write(&tab.file_name, &tab.input_box.join("\n")) {
                Ok(_) => {}
                Err(_) => {
                    return Ok(false);
                }
            }
            *mode = 0;
        }
        crossterm::event::KeyCode::Esc => {
            *mode = 0;
            the_command_line.clear();
        }
        _ => {}
    }
    Ok(true)
}

//////////////////////////////////////////// DEAD CODE BURIED HERE ////////////////////////////////////////////
