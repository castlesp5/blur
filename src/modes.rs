use crate::helpers::{Tab, log};

use crate::controls::{controls, default_controls};

pub fn normal_mode(
    tab: &mut Tab,
    event_key: crossterm::event::KeyEvent,
    mode: &mut i32,
    the_command_line: &mut String,
    splitted: &mut Vec<&str>,
) -> std::io::Result<bool> {
    if controls(
        event_key,
        &mut tab.cursor_y,
        &mut tab.cursor_x,
        &mut tab.gcursor,
        splitted,
    )
    .unwrap()
    {
        return Ok(true);
    }
    match event_key.code {
        crossterm::event::KeyCode::Char('a') => {
            let mut offset = 0;
            for y in 0..tab.cursor_y as usize {
                offset += splitted[y].len() as i32 + 1;
            }
            offset += splitted[tab.cursor_y as usize].len() as i32;
            if tab.gcursor < offset {
                tab.cursor_x += 1;
                tab.gcursor += 1;
            }
            *mode = 1;
        }
        crossterm::event::KeyCode::Char('i') => {
            *mode = 1;
        }
        crossterm::event::KeyCode::Char('e') => {
            let start = tab.cursor_x as usize;
            let mut offset = 0;
            let new_x = match splitted[tab.cursor_y as usize][start..].find(' ') {
                Some(rel) => start + rel + 1,
                None => splitted[tab.cursor_y as usize].len(),
            } as i32;
            for y in 0..tab.cursor_y as usize {
                offset += splitted[y].len() as i32 + 1;
            }

            tab.gcursor = offset + new_x;
            tab.cursor_x = new_x;
        }
        crossterm::event::KeyCode::Char('b') => {
            let start = tab.cursor_x as usize;
            let before = &splitted[tab.cursor_y as usize][..start];
            // let trimmed_len = before.trim_end_matches(' ').len();
            // before = &before[..trimmed_len];
            let new_x = match before.rfind(' ') {
                Some(rel) => rel,
                None => 0,
            } as i32;

            tab.gcursor += new_x - tab.cursor_x;
            tab.cursor_x = new_x;
        }
        crossterm::event::KeyCode::Char('o') => {
            let mut offset = 0;
            for y in 0..tab.cursor_y as usize {
                offset += splitted[y].len() as i32 + 1;
            }
            offset += splitted[tab.cursor_y as usize].len() as i32;
            tab.gcursor = offset;
            tab.input_box.insert(tab.gcursor as usize, '\n');
            tab.gcursor += 1;
            tab.cursor_y += 1;
            tab.cursor_x = 0;
            *mode = 1;
        }
        crossterm::event::KeyCode::Char('q') => {
            return Ok(false);
        }
        crossterm::event::KeyCode::Char('w') => {
            if tab.file_name.len() > 0 {
                match std::fs::write(&tab.file_name, &tab.input_box) {
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
            if tab.gcursor < tab.input_box.len() as i32 {
                tab.input_box.remove(tab.gcursor as usize);
            }
        }
        crossterm::event::KeyCode::Backspace => {
            if tab.gcursor > 0 {
                tab.gcursor -= 1;
                tab.input_box.remove(tab.gcursor as usize);

                if tab.cursor_x > 0 {
                    tab.cursor_x -= 1;
                } else if tab.cursor_y > 0 {
                    tab.cursor_y -= 1;
                    tab.cursor_x = splitted[tab.cursor_y as usize].len() as i32;
                }
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
    splitted: &mut Vec<&str>,
) -> std::io::Result<bool> {
    if default_controls(
        event_key,
        &mut tab.cursor_y,
        &mut tab.cursor_x,
        &mut tab.gcursor,
        splitted,
    )? {
        return Ok(true);
    }
    let gcursor = tab
        .input_box
        .char_indices()
        .nth(tab.gcursor as usize)
        .map_or(tab.input_box.len(), |(i, _)| i) as usize;

    match event_key.code {
        crossterm::event::KeyCode::Esc => {
            *mode = 0;
            return Ok(false);
        }

        crossterm::event::KeyCode::Char(c) => {
            tab.input_box.insert(gcursor, c);
            tab.cursor_x += 1;
            tab.gcursor += 1;
        }

        crossterm::event::KeyCode::Enter => {
            tab.input_box.insert(gcursor, '\n');
            tab.cursor_x = 0;
            tab.cursor_y += 1;
            tab.gcursor += 1;
        }
        crossterm::event::KeyCode::Tab => {
            tab.input_box.insert_str(gcursor, "    ");
            tab.cursor_x += 4;
            tab.gcursor += 4;
        }
        crossterm::event::KeyCode::Delete => {
            if tab.gcursor < tab.input_box.len() as i32 {
                tab.input_box.remove(gcursor);
            }
        }
        crossterm::event::KeyCode::Backspace => {
            if tab.gcursor > 0 {
                tab.gcursor -= 1;
                tab.input_box.remove(gcursor);

                if tab.cursor_x > 0 {
                    tab.cursor_x -= 1;
                } else if tab.cursor_y > 0 {
                    tab.cursor_y -= 1;
                    tab.cursor_x = splitted[tab.cursor_y as usize].len() as i32;
                }
            }
        }
        _ => {}
    }
    Ok(true)
}

pub fn insert_paste(tab: &mut Tab, text: &str) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");

    tab.input_box.insert_str(tab.gcursor as usize, &text);

    if let Some(last_newline) = text.rfind('\n') {
        let newline_count = text.matches('\n').count() as i32;
        tab.cursor_y += newline_count;
        tab.cursor_x = (text.len() - last_newline - 1) as i32;
    } else {
        tab.cursor_x += text.len() as i32;
    }
    tab.gcursor += text.len() as i32;
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
            tab.input_box.clear();
            match std::fs::read_to_string(&the_command_line) {
                Ok(content) => tab.input_box = content.clone(),
                Err(_) => {
                    return Ok(false);
                }
            }
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
            match std::fs::write(&tab.file_name, &tab.input_box) {
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
