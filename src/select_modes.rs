use crate::controls::controls;
use crate::helpers::{EditRecord, Tab, Visual};

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
        crossterm::event::KeyCode::Char('g') => {                                      
            tab.cursor_y = 0;                                                     
            tab.cursor_x = 0;                                                     
        }                                                                         
        crossterm::event::KeyCode::Char('G') => {                                 
            tab.cursor_y = tab.input_box.len() as i32 - 1;                        
            tab.cursor_x = 0;                                                     
        }  
        crossterm::event::KeyCode::Char('K') =>
        {
            if tab.cursor_y as usize > vis.v_y
            {
                if vis.v_y > 0
                {
                    let line = tab.input_box.remove(vis.v_y - 1);
                    tab.unsave();
                    tab.input_box.insert(tab.cursor_y as usize , line.clone());
                    vis.v_y -= 1;
                    tab.cursor_y -= 1;
                }
            }
            else if vis.v_y > tab.cursor_y as usize
            {
                if tab.cursor_y > 0
                {
                    let line = tab.input_box.remove(tab.cursor_y as usize - 1);
                    tab.unsave();
                    tab.input_box.insert(vis.v_y , line.clone());
                    vis.v_y -= 1;
                    tab.cursor_y -= 1;
                }
            }
            else                                                                                   
            {                                                                                      
                if (tab.cursor_y as usize) > 0                                             
                {                                                                                                
                    let text = tab.input_box.remove(tab.cursor_y as usize);
                    tab.unsave();
                    tab.input_box.insert(tab.cursor_y as usize - 1, text);
                    vis.v_y -= 1;
                    tab.cursor_y -= 1;
                }
            }
        }
        crossterm::event::KeyCode::Char('J') =>
        {
            if tab.cursor_y as usize > vis.v_y 
            {
                if (tab.cursor_y as usize) < tab.input_box.len() - 1
                {
                    let line = tab.input_box.remove(tab.cursor_y as usize + 1);
                    tab.unsave();
                    tab.input_box.insert(vis.v_y, line.clone());
                    vis.v_y += 1;
                    tab.cursor_y += 1;
                }
            }
            else if vis.v_y > tab.cursor_y as usize
            {
                if vis.v_y < tab.input_box.len() - 1
                {
                    let line = tab.input_box.remove(vis.v_y + 1);
                    tab.unsave();
                    tab.input_box.insert(tab.cursor_y as usize, line.clone());
                    vis.v_y += 1;
                    tab.cursor_y += 1;
                }
            }
            else
            {
                if (tab.cursor_y as usize) < tab.input_box.len() - 1                                             
                {                                                                                                
                    let text = tab.input_box.remove(tab.cursor_y as usize);                                      
                    tab.unsave();                                                                                
                    tab.input_box.insert(tab.cursor_y as usize + 1, text);                                       
                    vis.v_y += 1;
                    tab.cursor_y += 1;                                                                           
                }   
            }
        }
        crossterm::event::KeyCode::Char('>') => {
            tab.unsave();
            if tab.cursor_y as usize > vis.v_y
            {
                for i in vis.v_y..tab.cursor_y as usize + 1
                {
                    tab.undo_stack.push(EditRecord::InsertString {
                        row: i,
                        col: 0,
                        text: "    ".to_string(),
                    });
                    tab.redo_stack.clear();
                    tab.input_box[i].insert_str(0, "    ");
                }
            }
            else 
            {
                for i in tab.cursor_y as usize..vis.v_y + 1
                {
                    tab.undo_stack.push(EditRecord::InsertString {
                        row: i,
                        col: 0,
                        text: "    ".to_string(),
                    });
                    tab.redo_stack.clear();
                    tab.input_box[i].insert_str(0, "    ");
                }
            }
            tab.cursor_x = tab.input_box[tab.cursor_y as usize].find(|c: char| c != ' ').unwrap_or(0) as i32;
        }
        crossterm::event::KeyCode::Char('<') => {
            tab.unsave();
            if tab.cursor_y as usize > vis.v_y
            {
                for i in vis.v_y..tab.cursor_y as usize + 1
                {
                    if tab.input_box[i].starts_with("    ")
                    {
                        tab.undo_stack.push(EditRecord::RemoveString {
                            row: i,
                            col: 0,
                            text: "    ".to_string(),
                        });
                        tab.redo_stack.clear();
                        tab.input_box[i].replace_range(0..4, "");
                    }
                }
            }
            else
            {
                for i in tab.cursor_y as usize..vis.v_y + 1
                {
                    if tab.input_box[i].starts_with("    ")
                    {
                        tab.undo_stack.push(EditRecord::RemoveString {
                            row: i,
                            col: 0,
                            text: "    ".to_string(),
                        });
                        tab.redo_stack.clear();
                        tab.input_box[i].replace_range(0..4, "");
                    }
                }
            }
            tab.cursor_x = tab.input_box[tab.cursor_y as usize].find(|c: char| c != ' ').unwrap_or(0) as i32;
        }
        crossterm::event::KeyCode::Char('d') => {
            tab.unsave();
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
            }
                // tab.cursor_y = vis.v_y as i32;
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
            return Ok(false);
        }
        _ => {*mode = 0}
    }
    Ok(true)
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
        crossterm::event::KeyCode::Char('>') => {
            if tab.cursor_x < vis.v_x as i32
            {
                let line = &mut tab.input_box[tab.cursor_y as usize];
                let character = line.remove(vis.v_x + 1);
                line.insert(tab.cursor_x as usize - 1, character);
            }
            else if vis.v_x < tab.cursor_x as usize
            {
                let line = &mut tab.input_box[tab.cursor_y as usize];
                let character = line.remove(tab.cursor_x as usize + 1);
                line.insert(vis.v_x - 1, character);
            }
        }
        crossterm::event::KeyCode::Char('e') => {
            let start = tab.cursor_x as usize;
            let new_x = match tab.input_box[tab.cursor_y as usize][start..].find(' ') {
                Some(rel) => start + rel + 1,
                None => tab.input_box[tab.cursor_y as usize].len(),
            } as i32;

            tab.cursor_x = new_x;
        }
        crossterm::event::KeyCode::Char('b') => {
            let start = tab.cursor_x as usize;
            let before = &tab.input_box[tab.cursor_y as usize][..start];
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
        crossterm::event::KeyCode::Char('d') => {
            if tab.cursor_y as usize == vis.v_y
            {
                let line = &mut tab.input_box[tab.cursor_y as usize];

                tab.saved = false;
                tab.highlight_cache = None;
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
            return Ok(false);
        }
        _ => {*mode = 0}
    }
    Ok(true)
}
