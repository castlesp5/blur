use crate::controls::default_controls;
use crate::helpers::{EditRecord, Tab};

pub fn insert_mode(
    tab: &mut Tab,
    event_key: crossterm::event::KeyEvent,
    mode: &mut i32,
    text: &mut [String],
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
            tab.unsave();
            let row = tab.cursor_y as usize;
            let mut col = tab.cursor_x as usize;
            // electric closer: typing } ) ] on a blank indent dedents first,
            // like zed and vscode. flush the batch so undo stays ordered.
            if (c == '}' || c == ')' || c == ']') && col >= 4 {
                let prefix = tab.input_box[row][..col.min(tab.input_box[row].len())].to_string();
                if !prefix.is_empty() && prefix.chars().all(|ch| ch == ' ') {
                    if !filled_now.is_empty() {
                        let start = col - filled_now.len();
                        tab.undo_stack.push(EditRecord::InsertString {
                            row,
                            col: start,
                            text: filled_now.clone(),
                        });
                        tab.redo_stack.clear();
                    }
                    filled_now.clear();
                    col -= 4;
                    tab.input_box[row].replace_range(col..col + 4, "");
                    tab.undo_stack.push(EditRecord::RemoveString {
                        row,
                        col,
                        text: "    ".to_string(),
                    });
                    tab.redo_stack.clear();
                }
            }
            tab.input_box[row].insert(col, c);
            filled_now.push(c);
            tab.redo_stack.clear();
            tab.cursor_x = col as i32 + blen;
        }

        // shift-tab unindents one level in insert mode
        crossterm::event::KeyCode::BackTab => {
            let row = tab.cursor_y as usize;
            let col = tab.cursor_x as usize;
            if col >= 4 {
                let from = col - 4;
                if tab.input_box[row].get(from..col) == Some("    ") {
                    tab.input_box[row].replace_range(from..col, "");
                    tab.undo_stack.push(EditRecord::RemoveString {
                        row,
                        col: from,
                        text: "    ".to_string(),
                    });
                    tab.redo_stack.clear();
                    tab.unsave();
                    tab.cursor_x -= 4;
                }
            }
        }

        crossterm::event::KeyCode::Enter => {
            let y = tab.cursor_y as usize;
            let x = tab.cursor_x as usize;
            tab.unsave();
            // auto-indent: carry current indent, add a level after
            // openers like { ( [ and : (python, rust, c family)
            let cur = tab.input_box[y].clone();
            let at = x.min(cur.len());
            let at = if cur.is_char_boundary(at) {
                at
            } else {
                cur.floor_char_boundary(at)
            };
            let base: String = cur
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            let opener = cur.trim_end().ends_with('{')
                || cur.trim_end().ends_with('(')
                || cur.trim_end().ends_with('[')
                || cur.trim_end().ends_with(':');
            let mut next = base.clone();
            if opener {
                next.push_str("    ");
            }
            let tail = cur[at..].trim_start().to_string();
            let prev_ch = cur[..at].chars().next_back();
            let brace_pair = matches!(
                (prev_ch, tail.chars().next()),
                (Some('{'), Some('}')) | (Some('('), Some(')')) | (Some('['), Some(']'))
            );
            if brace_pair {
                // {|}| becomes three lines with the cursor on the
                // indented middle. one undo unit via BraceSplit.
                tab.input_box[y].truncate(at);
                tab.input_box.insert(y + 1, next.clone());
                tab.input_box.insert(y + 2, format!("{base}{tail}"));
                tab.undo_stack.push(EditRecord::BraceSplit {
                    row: y,
                    col: at,
                    indent: next.clone(),
                    base,
                    tail,
                });
                tab.redo_stack.clear();
                tab.cursor_x = next.len() as i32;
                tab.cursor_y += 1;
                return Ok(true);
            }
            // a lone closer dedents one level
            if tail.starts_with('}') || tail.starts_with(')') || tail.starts_with(']') {
                next.truncate(next.len().saturating_sub(4));
            }
            tab.input_box[y].truncate(at);
            tab.input_box.insert(y + 1, format!("{next}{tail}"));
            tab.undo_stack
                .push(EditRecord::SplitLine { row: y, col: at });
            tab.redo_stack.clear();
            tab.cursor_x = next.len() as i32;
            tab.cursor_y += 1;
        }
        crossterm::event::KeyCode::Tab => {
            tab.unsave();
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
        _ => {
            *mode = 0;
        }
    }
    Ok(true)
}

pub fn insert_paste(tab: &mut Tab, text: &str) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");

    tab.unsave();

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
    tabs: &mut Vec<Tab>,
    tab_selector: &mut usize,
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
            // opens into a fresh tab so the current buffer survives,
            // and a failed read leaves every existing tab untouched
            let path = the_command_line.clone();
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => return Ok(false),
            };
            let mut new_tab = Tab::new();
            new_tab.input_box = content.split('\n').map(str::to_string).collect();
            new_tab.file_name = path;
            new_tab.saved = true;
            new_tab.lang = String::new();
            new_tab.highlight_cache = None;
            tabs.push(new_tab);
            *tab_selector = tabs.len() - 1;
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
            match std::fs::write(&the_command_line, tab.input_box.join("\n")) {
                Ok(_) => {
                    // the name drives language detection, so the cached
                    // spans from before the save are now wrong
                    tab.file_name = the_command_line.clone();
                    tab.saved = true;
                    tab.lang = String::new();
                    tab.highlight_cache = None;
                    the_command_line.clear();
                    *mode = 0;
                }
                Err(_) => {
                    // keep the typed path so the user can fix it and retry
                    return Ok(false);
                }
            }
        }
        crossterm::event::KeyCode::Esc => {
            *mode = 0;
            the_command_line.clear();
        }
        _ => {}
    }
    Ok(true)
}
