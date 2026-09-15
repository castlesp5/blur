use crate::helpers::{Tab, log};

use crate::controls::{controls, default_controls};

pub fn normal_mode(
    tab: &mut Tab,
    event_key: crossterm::event::KeyEvent,
    mode: &mut i32,
    the_command_line: &mut String,
    text: &mut Vec<String>,
) -> std::io::Result<bool> {
    if controls(event_key, &mut tab.cursor_y, &mut tab.cursor_x, text).unwrap() {
        return Ok(true);
    }
    match event_key.code {
        crossterm::event::KeyCode::Char('a') => {
            let mut offset = 0;
            for y in 0..tab.cursor_y as usize {
                offset += text[y].len() as i32 + 1;
            }
            if (tab.cursor_x as usize) < text[tab.cursor_y as usize].len() {
                tab.cursor_x += 1;
            }
            *mode = 1;
        }
        crossterm::event::KeyCode::Char('i') => {
            *mode = 1;
        }
        crossterm::event::KeyCode::Char('e') => {
            let start = tab.cursor_x as usize;
            let mut offset = 0;
            let new_x = match text[tab.cursor_y as usize][start..].find(' ') {
                Some(rel) => start + rel + 1,
                None => text[tab.cursor_y as usize].len(),
            } as i32;
            for y in 0..tab.cursor_y as usize {
                offset += text[y].len() as i32 + 1;
            }

            tab.cursor_x = new_x;
        }
        crossterm::event::KeyCode::Char('b') => {
            let start = tab.cursor_x as usize;
            let before = &text[tab.cursor_y as usize][..start];
            // let trimmed_len = before.trim_end_matches(' ').len();
            // before = &before[..trimmed_len];
            let new_x = match before.rfind(' ') {
                Some(rel) => rel,
                None => 0,
            } as i32;

            tab.cursor_x = new_x;
        }
        crossterm::event::KeyCode::Char('o') => {
            let mut offset = 0;
            for y in 0..tab.cursor_y as usize {
                offset += text[y].len() as i32 + 1;
            }
            offset += text[tab.cursor_y as usize].len() as i32;
            tab.input_box.insert(tab.cursor_y as usize, String::new());
            tab.cursor_y += 1;
            tab.cursor_x = 0;
            *mode = 1;
        }
        crossterm::event::KeyCode::Char('q') => {
            return Ok(false);
        }
        crossterm::event::KeyCode::Char('w') => {
            if tab.file_name.len() > 0 {
                match std::fs::write(&tab.file_name, &tab.input_box.join("\n")) {
                    Ok(_) => {}
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
            if x < tab.input_box[y].len() {
                tab.input_box[y].remove(x);
            }
        }
        crossterm::event::KeyCode::Backspace => {
            if tab.cursor_x > 0 {
                let x = tab.cursor_x as usize;
                let y = tab.cursor_y as usize;
                tab.input_box[y].remove(x - 1);
                tab.cursor_x -= 1;
            } else if tab.cursor_y > 0 {
                tab.cursor_y -= 1;
                tab.cursor_x = text[tab.cursor_y as usize].len() as i32;
            }
        }
        _ => {}
    }
    Ok(true)
}

pub fn insert_mode(
    tab: &mut Tab,
    event_key: crossterm::event::KeyEvent,
    mode: &mut i32,
    text: &mut Vec<String>,
) -> std::io::Result<bool> {
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
            tab.input_box[tab.cursor_y as usize].insert(tab.cursor_x as usize, c);
            tab.cursor_x += blen;
        }

        crossterm::event::KeyCode::Enter => {
            let y = tab.cursor_y as usize;
            let x = tab.cursor_x as usize;
            let rest = tab.input_box[y].split_off(x);
            tab.input_box.insert(y + 1, rest);
            tab.cursor_x = 0;
            tab.cursor_y += 1;
        }
        crossterm::event::KeyCode::Tab => {
            tab.input_box[tab.cursor_y as usize].insert_str(tab.cursor_x as usize, "    ");
            tab.cursor_x += 4;
        }
        crossterm::event::KeyCode::Delete => {
            let y = tab.cursor_y as usize;
            let x = tab.cursor_x as usize;
            if y < tab.input_box.len() && x < tab.input_box[y].len() {
                tab.input_box[y].remove(x);
            }
        }
        crossterm::event::KeyCode::Backspace => {
            let y = tab.cursor_y as usize;
            if tab.cursor_x > 0 {
                let x = tab.cursor_x as usize;
                if let Some((prev_idx, ch)) = tab.input_box[y][..x].char_indices().next_back() {
                    tab.input_box[y].remove(prev_idx);
                    tab.cursor_x -= ch.len_utf8() as i32;
                }
            } else if tab.cursor_y > 0 {
                tab.cursor_y -= 1;
                tab.cursor_x = text[tab.cursor_y as usize].len() as i32;
            }
        }
        _ => {}
    }
    Ok(true)
}

pub fn insert_paste(tab: &mut Tab, text: &str) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");

    tab.input_box[tab.cursor_y as usize].insert_str(tab.cursor_x as usize, &text);

    if let Some(last_newline) = text.rfind('\n') {
        let newline_count = text.matches('\n').count() as i32;
        tab.cursor_y += newline_count;
        tab.cursor_x = (text.len() - last_newline - 1) as i32;
    } else {
        tab.cursor_x += text.len() as i32;
    }
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

// pub fn command_mode(
//     event_key: crossterm::event::KeyEvent,
//     mode: &mut i32,
//     the_command_line: &mut String,
//     input_box: &mut String
//     ) -> std::io::Result<bool>
// {
//     match mode {
//         10 =>
//         {
//             std::fs::write("output.txt", input_box)?;
//         }
//         _ => {}
//     }
//     Ok(true)
// }
//
//
// pub fn execute_commands(
//     the_command_line: &mut String
// ) -> std::io::Result<bool>
// {
//     let mut parts: Vec<&str> = the_command_line.split_whitespace().collect();
//
//     let output = std::process::Command::new(parts[0])
//         .args(&parts[1..])
//         .output()
//         .expect("Failed to run the command");
//
//     println!("{:?}", output);
//     the_command_line.clear();
//     Ok(true)
// }
