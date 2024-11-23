use crate::Position;
use std::io::{self, stdout, Write};
use crossterm::{
    cursor,
    event::{self, read, Event, KeyEvent, MouseEvent},
    execute,
    style::{Color, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

// use crossterm::event::KeyCode;
use crossterm::terminal::Clear;

pub struct Size {
    pub width: u16,
    pub height: u16,
}

pub struct Terminal {
    size: Size,
}

impl Terminal {
    pub fn default() -> Result<Self, std::io::Error> {
        let (width, height) = terminal::size()?;
        execute!(stdout(), EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;
        Ok(Self {
            size: Size {
                width,
                height: height.saturating_sub(2),
            },
        })
    }

    pub fn size(&self) -> &Size {
        &self.size
    }

    pub fn clear_screen() {
        execute!(stdout(), Clear(ClearType::All)).unwrap();
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn cursor_position(position: &Position) {
        let Position { mut x, mut y } = position;
        x = x.saturating_add(0);// 更改光标显示位置
        y = y.saturating_add(0);
        let x = x as u16;
        let y = y as u16;
        execute!(stdout(), cursor::MoveTo(x, y)).unwrap();
    }

    pub fn flush() -> Result<(), std::io::Error> {
        io::stdout().flush()
    }

    pub fn read_key() -> Result<KeyEvent, std::io::Error> {
        loop {
            if event::poll(std::time::Duration::from_millis(50))? {
                if let event::Event::Key(key) = event::read()? {
                    return Ok(key);
                }
            
            }
        }
    }

    
    pub fn read(&mut self) -> Result<Event, std::io::Error> {
        loop {
            let event = read();

            if let Ok(Event::Key(_)) = event {
                // self.cycle_colors();
            }
            return event
        }
    }

    // pub fn read_mouse() -> Result<MouseEvent, std::io::Error> {
    //     loop {
    //         if event::poll(std::time::Duration::from_millis(50))? {
    //             if let event::Event::Mouse(mouse) = event::read()? {
    //                 return Ok(mouse);
    //             }
    //         }
    //     }
    // }

    pub fn cursor_hide() {
        execute!(stdout(), cursor::Hide).unwrap();
    }

    pub fn cursor_show() {
        execute!(stdout(), cursor::Show).unwrap();
    }

    pub fn clear_current_line() {
        execute!(stdout(), Clear(ClearType::CurrentLine)).unwrap();
    }

    pub fn set_bg_color(color: Color) {
        execute!(stdout(), SetBackgroundColor(color)).unwrap();
    }

    pub fn reset_bg_color() {
        execute!(stdout(), SetBackgroundColor(Color::Reset)).unwrap();
    }

    pub fn set_fg_color(color: Color) {
        execute!(stdout(), SetForegroundColor(color)).unwrap();
    }

    pub fn reset_fg_color() {
        execute!(stdout(), SetForegroundColor(Color::Reset)).unwrap();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        execute!(stdout(), LeaveAlternateScreen).unwrap();
        terminal::disable_raw_mode().unwrap();
    }
}