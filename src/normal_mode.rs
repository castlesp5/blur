use crate::controls::{controls};
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
    let tab = &mut tabs[*tab_selector];
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
        crossterm::event::KeyCode::Char('g') => {
            tab.cursor_y = 0;
            tab.cursor_x = 0;
        }
        crossterm::event::KeyCode::Char('G') => {
            tab.cursor_y = tab.input_box.len() as i32 - 1;
            tab.cursor_x = 0;
        }
        crossterm::event::KeyCode::Char('a') => {
            if (tab.cursor_x as usize) < text[tab.cursor_y as usize].len() {
                tab.cursor_x += 1;
            }
            *mode = 1;
        }
        crossterm::event::KeyCode::Char('i') => {
            *mode = 1; } crossterm::event::KeyCode::Char('K') => { if tab.cursor_y as usize > 0 {
                let text = tab.input_box.remove(tab.cursor_y as usize);
                tab.unsave();
                tab.input_box.insert(tab.cursor_y as usize - 1, text);
                tab.cursor_y -= 1
            }
        }
        crossterm::event::KeyCode::Char('J') => {
            if (tab.cursor_y as usize) < tab.input_box.len() - 1
            {
                let text = tab.input_box.remove(tab.cursor_y as usize);
                tab.unsave();
                tab.input_box.insert(tab.cursor_y as usize + 1, text);
                tab.cursor_y += 1;
            }
        }
        crossterm::event::KeyCode::Char('>') => {
            tab.unsave();
            tab.undo_stack.push(EditRecord::InsertString {
                row: tab.cursor_y as usize,
                col: 0,
                text: "    ".to_string(),
            });
            tab.redo_stack.clear();
            tab.input_box[tab.cursor_y as usize].insert_str(0, "    ");
            tab.cursor_x = tab.input_box[tab.cursor_y as usize].find(|c: char| c != ' ').unwrap_or(0) as i32;
            crate::helpers::log("test");
        }
        crossterm::event::KeyCode::Char('<') => {
            tab.unsave();
            if tab.input_box[tab.cursor_y as usize].starts_with("    ")
            {
                tab.undo_stack.push(EditRecord::RemoveString {
                    row: tab.cursor_y as usize,
                    col: 0,
                    text: "    ".to_string(),
                });
                tab.redo_stack.clear();
                tab.input_box[tab.cursor_y as usize].replace_range(0..4, "");
            }
            tab.cursor_x = tab.input_box[tab.cursor_y as usize].find(|c: char| c != ' ').unwrap_or(0) as i32;
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
        crossterm::event::KeyCode::Char('E') => {
            tab.cursor_x = tab.input_box[tab.cursor_y as usize].len() as i32;
        }
        crossterm::event::KeyCode::Char('B') => {
            tab.cursor_x = 0;
        }
        crossterm::event::KeyCode::Char('o') => {
            tab.unsave();
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
            tab.unsave();
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
            tab.unsave();
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
            tab.unsave();
            if tab.input_box.len() == 0 {
                tab.input_box.push(String::new());
            }
            if y == tab.input_box.len()
            {
                tab.cursor_y -= 1;
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
                tab.unsave();
                tab.cursor_y = row as i32;
                tab.cursor_x = col as i32;
                tab.redo_stack.push(record);
            }
        }
        crossterm::event::KeyCode::Char('r') => {
            if let Some(record) = tab.redo_stack.pop() {
                let (row, col) = apply_forward(&record, &mut tab.input_box);
                tab.unsave();
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
