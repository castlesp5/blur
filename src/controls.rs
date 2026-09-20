pub fn default_controls(
    event_key: crossterm::event::KeyEvent,
    cursor_y: &mut i32,
    cursor_x: &mut i32,
    text: &Vec<String>,
) -> std::io::Result<bool> {
    let x = *cursor_x as usize;
    let y = *cursor_y as usize;
    match event_key.code {
        crossterm::event::KeyCode::Left => {
            if x > 0
            {
                if let Some((_ , ch)) = text[y][..x].char_indices().next_back() {
                    *cursor_x -= ch.len_utf8() as i32;
                }
            }
        }
        crossterm::event::KeyCode::Right => {
            if x < text[*cursor_y as usize].len() {
                if let Some((_ , ch)) = text[y][x..].char_indices().next() {
                    *cursor_x += ch.len_utf8() as i32;
                }
            }
        }
        crossterm::event::KeyCode::Up => {
            if y > 0 {
                if x > text[y - 1].len() {
                    *cursor_x = text[y - 1].len() as i32;
                }
                else {
                    while !text[y - 1].is_char_boundary(*cursor_x as usize)
                    {
                        *cursor_x += 1;
                    }
                }

                *cursor_y -= 1;
            }
        }

        crossterm::event::KeyCode::Down => {
            if y + 1 < text.len() {
                if x > text[y + 1].len() {
                    *cursor_x = text[y + 1].len() as i32;
                }
                else {
                    while !text[y + 1].is_char_boundary(*cursor_x as usize)
                    {
                        *cursor_x += 1;
                    }
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
    text: &Vec<String>,
) -> std::io::Result<bool> {
    if default_controls(event_key, cursor_y, cursor_x, text)? {
        return Ok(true);
    }
    let x = *cursor_x as usize;
    let y = *cursor_y as usize;
    match event_key.code {
        crossterm::event::KeyCode::Char('h') => {
            if x > 0
            {
                if let Some((_ , ch)) = text[y][..x].char_indices().next_back() {
                    *cursor_x -= ch.len_utf8() as i32;
                }
            }
        }
        crossterm::event::KeyCode::Char('l') => {
            if x < text[*cursor_y as usize].len() {
                if let Some((_ , ch)) = text[y][x..].char_indices().next() {
                    *cursor_x += ch.len_utf8() as i32;
                }
            }
        }
        crossterm::event::KeyCode::Char('k') => {
            if y > 0 {
                if x > text[y - 1].len() {
                    *cursor_x = text[y - 1].len() as i32;
                }
                else {
                    while !text[y - 1].is_char_boundary(*cursor_x as usize)
                    {
                        *cursor_x += 1;
                    }
                }

                *cursor_y -= 1;
            }
        }

        crossterm::event::KeyCode::Char('j') => {
            if y + 1 < text.len() {
                if x > text[y + 1].len() {
                    *cursor_x = text[y + 1].len() as i32;
                }
                else {
                    while !text[y + 1].is_char_boundary(*cursor_x as usize)
                    {
                        *cursor_x += 1;
                    }
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
