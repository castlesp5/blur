pub fn default_controls(
    event_key: crossterm::event::KeyEvent,
    cursor_y: &mut i32,
    cursor_x: &mut i32,
    text: &mut Vec<String>,
) -> std::io::Result<bool> {
    match event_key.code {
        crossterm::event::KeyCode::Left => {
            if *cursor_x > 0 {
                *cursor_x -= 1;
            }
        }
        crossterm::event::KeyCode::Right => {
            if *cursor_x < text[*cursor_y as usize].len() as i32 {
                *cursor_x += 1;
            }
        }
        crossterm::event::KeyCode::Up => {
            if *cursor_y > 0 {
                if *cursor_x > text[*cursor_y as usize - 1].len() as i32 {
                    *cursor_x = text[*cursor_y as usize - 1].len() as i32;
                }
                *cursor_y -= 1;
            }
        }
        crossterm::event::KeyCode::Down => {
            if *cursor_y < text.len() as i32 - 1 {
                if *cursor_x > text[*cursor_y as usize + 1].len() as i32 {
                    *cursor_x = text[*cursor_y as usize + 1].len() as i32;
                }
                *cursor_y += 1;
            }
        }
        _ => {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn controls(
    event_key: crossterm::event::KeyEvent,
    cursor_y: &mut i32,
    cursor_x: &mut i32,
    text: &mut Vec<String>,
) -> std::io::Result<bool> {
    if default_controls(event_key, cursor_y, cursor_x, text)? {
        return Ok(true);
    }
    match event_key.code {
        crossterm::event::KeyCode::Char('h') => {
            if *cursor_x > 0 {
                *cursor_x -= 1;
            }
        }
        crossterm::event::KeyCode::Char('l') => {
            if *cursor_x < text[*cursor_y as usize].len() as i32 {
                *cursor_x += 1;
            }
        }
        crossterm::event::KeyCode::Char('k') => {
            if *cursor_y > 0 {
                if *cursor_x > text[*cursor_y as usize - 1].len() as i32 {
                    *cursor_x = text[*cursor_y as usize - 1].len() as i32;
                }
                *cursor_y -= 1;
            }
        }
        crossterm::event::KeyCode::Char('j') => {
            if *cursor_y < text.len() as i32 - 1 {
                if *cursor_x > text[*cursor_y as usize + 1].len() as i32 {
                    *cursor_x = text[*cursor_y as usize + 1].len() as i32;
                }
                *cursor_y += 1;
            }
        }
        _ => {
            return Ok(false);
        }
    }
    Ok(true)
}
