use crate::Document;
use crate::Row;
use crate::Terminal;
use std::env;
use std::time::Duration;
use std::time::Instant;
// use std::include_bytes;
// use crossterm::style::Colors
// use crossterm::event:：{Event,read}
use console::style;
use crossterm::{
    style::{Color, ResetColor, SetForegroundColor},
    event::{self,Event,KeyCode, KeyModifiers, KeyEvent, MouseEventKind, MouseButton, MouseEvent},
    ExecutableCommand,
};
use std::io::{self, Write};

const STATUS_FG_COLOR: Color = Color::Rgb { r: 63, g: 63, b: 63 };
const STATUS_BG_COLOR: Color = Color::Rgb { r: 239, g: 239, b: 239 };
const VERSION: &str = env!("CARGO_PKG_VERSION");
const QUIT_TIMES: u8 = 3;
const MAX_LINE_LEN: usize = 50;

#[derive(PartialEq, Copy, Clone)]
pub enum SearchDirection {
    Forward,
    Backward,
}

#[derive(Default, Clone)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

struct StatusMessage {
    text: String,
    time: Instant,
}

pub struct Language {
    pub name: String,
}

impl StatusMessage {
    fn from(message: String) -> Self {
        Self {
            time: Instant::now(),
            text: message,
        }
    }
}

pub struct Editor {
    should_quit: bool,
    terminal: Terminal,
    cursor_position: Position,
    offset: Position,
    document: Document,
    status_message: StatusMessage,
    quit_times: u8,
    highlighted_word: Option<String>,
}

impl Editor {
    pub fn run(&mut self) {
        std::io::stdout().execute(crossterm::event::EnableMouseCapture).unwrap();
        
        loop {
            if let Err(error) = self.refresh_screen() {
                die(error);
            }
            if self.should_quit {
                break;
            }
            if let Err(error) = self.process_keypress() {
                die(error);
            }
            // if let Err(error) = self.process_mousepress() {
            //     die(error);
            // }
        }
    }
    pub fn default() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut initial_status =
            String::from(format!("[{}]: Ctrl-F = find | Ctrl-S = save | Ctrl-Q = quit", style("Help").cyan()));

        let document = if let Some(file_name) = args.get(1) {
            let doc = Document::open(file_name);
            if let Ok(doc) = doc {
                doc
            } else {
                initial_status = format!("[{}] Could not open file: {}",style("Error").red(), file_name);
                Document::default()
            }
        } else {
            Document::default()
        };

        Self {
            should_quit: false,
            terminal: Terminal::default().expect(&format!("[{}] Failed to initialize terminal", style("Error").red())),
            document,
            cursor_position: Position::default(),
            offset: Position::default(),
            status_message: StatusMessage::from(initial_status),
            quit_times: QUIT_TIMES,
            highlighted_word: None,
        }
    }

    // fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
    //     match mouse_event.kind {
    //         MouseEventKind::Down(_) => {
    //             let (x, y) = (mouse_event.column, mouse_event.row);
    //             self.cursor_position.x = x as usize;
    //             self.cursor_position.y = y as usize;
    //         }
    //         _ => {}
    //     }
    // }

    fn refresh_screen(&mut self) -> Result<(), std::io::Error> {
        Terminal::cursor_hide();
        Terminal::cursor_position(&Position::default());
        if self.should_quit {
            Terminal::clear_screen();
            println!("Goodbye.\r");
        } else {
            self.document.highlight(
                &self.highlighted_word,
                Some(
                    self.offset
                        .y
                        .saturating_add(self.terminal.size().height as usize),
                ),
            );
            self.draw_rows();
            self.draw_status_bar();
            self.draw_message_bar();
            Terminal::cursor_position(&Position {
                x: self.cursor_position.x.saturating_sub(self.offset.x),
                y: self.cursor_position.y.saturating_sub(self.offset.y),
            });
        }
        Terminal::cursor_show();
        Terminal::flush()
    }

    fn save(&mut self) {
        if self.document.file_name.is_none() {
            let new_name = self.prompt("Save as: ", |_, _, _| {}).unwrap_or(None);
            if new_name.is_none() {
                self.status_message = StatusMessage::from("Save aborted.".to_string());
                return;
            }
            let mut file_name = new_name.unwrap();
            if !file_name.ends_with(".txt") {
                file_name.push_str(".txt");
            }
            self.document.file_name = Some(file_name);
        }

        if self.document.save().is_ok() {
            self.status_message = StatusMessage::from(format!("[{}] File save successfully!", style("Success").green()));
        } else {
            self.status_message = StatusMessage::from(format!("[{}] Error writing file!", style("Error").red()));
        }
    }

    fn search(&mut self) {
        let old_position = self.cursor_position.clone();
        let mut direction = SearchDirection::Forward;
        let query = self
            .prompt(
                "Search (ESC to cancel, Arrows to navigate): ",
                |editor, key, query| {
                    let mut moved = false;
                    match key {
                        KeyEvent {
                            code: KeyCode::Right | KeyCode::Down | KeyCode::Enter,
                            modifiers: KeyModifiers::NONE,
                        } => {
                            direction = SearchDirection::Forward;
                            editor.move_cursor(KeyCode::Right);
                            moved = true;
                        }
                        KeyEvent {
                            code: KeyCode::Left | KeyCode::Up,
                            modifiers: KeyModifiers::NONE,
                        } => direction = SearchDirection::Backward,
                        _ => direction = SearchDirection::Forward,
                    }
                    if let Some(position) =
                        editor
                            .document
                            .find(&query, &editor.cursor_position, direction)
                    {
                        editor.cursor_position = position;
                        editor.scroll();
                    } else if moved {
                        editor.move_cursor(KeyCode::Left);
                    }
                    editor.highlighted_word = Some(query.to_string());
                },
            )
            .unwrap_or(None);

        if query.is_none() {
            self.cursor_position = old_position;
            self.scroll();
        }
        self.highlighted_word = None;
    }

    // fn process_keypress(&mut self) -> Result<(), std::io::Error> {
    //     let pressed_key = Terminal::read_key()?;
    //     match pressed_key {
    //         KeyEvent {
    //             code: KeyCode::Char('q'),
    //             modifiers: KeyModifiers::CONTROL,
    //         } => {
    //             if self.quit_times > 0 && self.document.is_dirty() {
    //                 self.status_message = StatusMessage::from(format!(
    //                     "WARNING! File has unsaved changes. Press Ctrl-Q {} more times to quit.",
    //                     self.quit_times
    //                 ));
    //                 self.quit_times -= 1;
    //                 return Ok(());
    //             }
    //             self.should_quit = true
    //         }
    //         KeyEvent {
    //             code: KeyCode::Char('s'),
    //             modifiers: KeyModifiers::CONTROL,
    //         } => self.save(),
    //         KeyEvent {
    //             code: KeyCode::Char('f'),
    //             modifiers: KeyModifiers::CONTROL,
    //         } => self.search(),
    //         KeyEvent {
    //             code: KeyCode::Char(c),
    //             modifiers: KeyModifiers::NONE,
    //         } => {
    //             self.document.insert(&self.cursor_position, c);
    //             if self.cursor_position.x >= 50 {
    //                 self.document.insert(&self.cursor_position, '\n');
    //                 self.cursor_position.x = 0;
    //                 self.cursor_position.y += 1;
    //             }
    //             self.move_cursor(KeyCode::Right);
    //         }
    //         KeyEvent {
    //             code: KeyCode::Delete,
    //             modifiers: KeyModifiers::NONE,
    //         } => self.document.delete(&self.cursor_position),
    //         KeyEvent {
    //             code: KeyCode::Backspace,
    //             modifiers: KeyModifiers::NONE,
    //         } => {
    //             if self.cursor_position.x > 0 || self.cursor_position.y > 0 {
    //                 // self.move_cursor(KeyCode::Left);
    //                 if self.cursor_position.x > 0 {
    //                     self.cursor_position.x -= 1;
    //                 } else {
    //                     self.cursor_position.y -= 1;
    //                     self.cursor_position.x = self.document.row(self.cursor_position.y).expect("REASON").len();
    //                 }
    //                 self.document.delete(&self.cursor_position);
    //             }
    //         }
    //         KeyEvent {
    //             code: KeyCode::Up
    //             | KeyCode::Down
    //             | KeyCode::Left
    //             | KeyCode::Right
    //             | KeyCode::PageUp
    //             | KeyCode::PageDown
    //             | KeyCode::End
    //             | KeyCode::Enter
    //             | KeyCode::Home,
    //             modifiers: KeyModifiers::NONE,
    //         } => self.move_cursor(pressed_key.code),
    //         _ => (),
    //     }
    //     self.scroll();
    //     if self.quit_times < QUIT_TIMES {
    //         self.quit_times = QUIT_TIMES;
    //         self.status_message = StatusMessage::from(String::new());
    //     }
    //     Ok(())
    // }

    fn process_keypress(&mut self) -> Result<(), std::io::Error> {
        let event = Terminal::read(&mut self.terminal)?;
        if let Event::Mouse(mouse_event) = event {
            match mouse_event.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    self.cursor_position = Position {
                        x: mouse_event.column as usize,
                        y: mouse_event.row as usize,
                    };
                    // self.scroll();
                }
                _ => (), // 处理其他 MouseEventKind 的情况
            }
        }
        if let Event::Key(pressed_key) = event {
            match (pressed_key.modifiers, pressed_key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('q')) | (_, KeyCode::Esc) => {
                    if self.quit_times > 0 && self.document.is_dirty() {
                        self.status_message = StatusMessage::from(format!(
                            "[{}] File has unsaved changes. Press Ctrl-Q {} more times to quit.",
                            style("WARNING").red(),style(self.quit_times).cyan()
                        ));
                        self.quit_times -= 1;
                        return Ok(());
                    }
                    self.should_quit = true
                }
                (KeyModifiers::CONTROL, KeyCode::Char('s')) => self.save(),
                (KeyModifiers::CONTROL, KeyCode::Char('f')) => self.search(),
                (_, KeyCode::Enter) => {
                    self.document.insert(&self.cursor_position, '\n');
                    self.move_cursor(KeyCode::Right);
                },
                (_, KeyCode::Char(c)) => {
                    self.document.insert(&self.cursor_position, c);
                    if self.cursor_position.x >= MAX_LINE_LEN {
                        self.document.insert(&self.cursor_position, '\n');
                        self.cursor_position.x = 0;
                        self.cursor_position.y += 1;
                    } else {
                        let width = if let Some(row) = self.document.row(self.cursor_position.y) {
                            row.get_char_width(c)
                        } else {
                            1
                        };
                        self.cursor_position.x += width;
                    }
                }
                (_, KeyCode::Delete) => {
                    if let Some(row) = self.document.row(self.cursor_position.y) {
                        if self.cursor_position.x < row.len() {
                            // 获取当前位置字符的UTF-8宽度
                            if let Some(c) = row.get_char(self.cursor_position.x) {
                                let char_width = row.get_char_width(c);
                                self.document.delete(&self.cursor_position);
                            }
                        }
                    }
                },
                (_, KeyCode::Backspace) => {
                    if self.cursor_position.x > 0 || self.cursor_position.y > 0 {
                    // self.move_cursor(KeyCode::Left);
                    if self.cursor_position.x > 0 {
                        self.cursor_position.x -= 1;
                    } else {
                        self.cursor_position.y -= 1;
                        self.cursor_position.x = self.document.row(self.cursor_position.y).expect("REASON").len();
                    }
                    self.document.delete(&self.cursor_position);
                }
                },
                (_, KeyCode::Up)
                | (_, KeyCode::Down)
                | (_, KeyCode::Left)
                | (_, KeyCode::Right)
                | (_, KeyCode::PageUp)
                | (_, KeyCode::PageDown)
                | (_, KeyCode::End)
                | (_, KeyCode::Home) => self.move_cursor(pressed_key.code),
                _ => (),
            }
            self.scroll();
            if self.quit_times < QUIT_TIMES {
                self.quit_times = QUIT_TIMES;
                self.status_message = StatusMessage::from(String::new());
            }
        }

        // else if let Event::Resize(width, height) = event {
        //     self.terminal.size.width = width;
        //     if env::consts::OS == "windows" {
        //         self.terminal.size.height = height - 1;
        //     }
        //     else {
        //         self.terminal.size.height = height - 2;
        //     }
        // }


        Ok(())
    }

    fn scroll(&mut self) {
        let Position { x, y } = self.cursor_position;
        let width = self.terminal.size().width as usize;
        let height = self.terminal.size().height as usize;
        let offset = &mut self.offset;
        if y < offset.y {
            offset.y = y;
        } else if y >= offset.y.saturating_add(height) {
            offset.y = y.saturating_sub(height).saturating_add(1);
        }
        if x < offset.x {
            offset.x = x;
        } else if x >= offset.x.saturating_add(width) {
            offset.x = x.saturating_sub(width).saturating_add(1);
        }
    }
    fn move_cursor(&mut self, key: KeyCode) {
        let terminal_height = self.terminal.size().height as usize;
        let Position { mut y, mut x } = self.cursor_position;
        let height = self.document.len();
        let mut width = if let Some(row) = self.document.row(y) {
            row.get_width_to(row.len())
        } else {
            0
        };

        match key {
            KeyCode::Up => y = y.saturating_sub(1),
            
            KeyCode::Down => {
                if y < height {
                    self.document.insert(&self.cursor_position, '\n');
                    y = y.saturating_add(1);
                }
            }
            KeyCode::Left => {
                if x > 0 {
                    if let Some(row) = self.document.row(y) {
                        let char_index = row.get_char_index(x);
                        if char_index > 0 {
                            if let Some(c) = row.get_char(char_index - 1) {
                                x -= row.get_char_width(c);
                            }
                        }
                    }
                } else if y > 0 {
                    y -= 1;
                    if let Some(row) = self.document.row(y) {
                        x = row.get_width_to(row.len());
                    } else {
                        x = 0;
                    }
                }
            }
            KeyCode::Right => {
                if let Some(row) = self.document.row(y) {
                    let char_index = row.get_char_index(x);
                    if char_index < row.len() {
                        if let Some(c) = row.get_char(char_index) {
                            x += row.get_char_width(c);
                        }
                    } else if y < height {
                        y += 1;
                        x = 0;
                    }
                }
            }
            KeyCode::PageUp => {
                y = if y > terminal_height {
                    y.saturating_sub(terminal_height)
                } else {
                    0
                }
            }
            KeyCode::PageDown => {
                y = if y.saturating_add(terminal_height) < height {
                    y.saturating_add(terminal_height)
                } else {
                    height
                }
            }
            KeyCode::Enter => {
                if y < height {
                    y = y.saturating_add(1);
                }
            }
            KeyCode::Home => x = 0,
            KeyCode::End => x = width,
            _ => (),
        }
        width = if let Some(row) = self.document.row(y) {
            row.get_width_to(row.len())
        } else {
            0
        };
        if x > width {
            x = width;
        }

        self.cursor_position = Position { x, y }
    }

    fn draw_welcome_message(&self) {
        let mut welcome_message = format!(
            "{}{}{}{}{} editor -- version {}",
            style("H").red(),
            style("e").yellow(),
            style("c").green(),
            style("t").blue(),
            style("o").magenta(),
            style(VERSION).cyan()
        );
        let width = self.terminal.size().width as usize;
        let len = welcome_message.len();
        #[allow(clippy::integer_arithmetic, clippy::integer_division)]
        let padding = width.saturating_sub(len) / 2;
        let spaces = " ".repeat(padding);
        welcome_message = format!("{}{}", spaces, welcome_message);
        welcome_message.truncate(width);
        // 设置颜色
        // print!("{}", SetForegroundColor(Color::Blue));
        println!("{}\r", welcome_message);
        io::stdout().flush().unwrap();
    }

    pub fn draw_row(&self, row: &Row) {
        let width = self.terminal.size().width as usize;
        let start = self.offset.x;
        let end = self.offset.x.saturating_add(width);
        let row = row.render(start, end);
        println!("{}\r", row)
    }

    #[allow(clippy::integer_division, clippy::integer_arithmetic)]
    fn draw_rows(&self) {
        let height = self.terminal.size().height;
        for terminal_row in 0..height {
            Terminal::clear_current_line();
            if let Some(row) = self
                .document
                .row(self.offset.y.saturating_add(terminal_row as usize))
            {
                self.draw_row(row);
            } else if self.document.is_empty() && terminal_row == height / 3 {
                self.draw_welcome_message();
            } else if !self.document.is_empty() {
                let line_number = self.offset.y.saturating_add(terminal_row as usize) + 1;
                print!("{}", SetForegroundColor(Color::Rgb {r: 252, g: 196, b: 228}));
                println!("{}\r", line_number);
                print!("{}", ResetColor);
            }//进入编辑模式
            else {
                println!("~\r");
            }
        }
    }

    fn draw_status_bar(&self) {
        let mut status;
        let width = self.terminal.size().width as usize;
        let modified_indicator = if self.document.is_dirty() {
            " (modified)"
        } else {
            ""
        };

        let mut file_name = "[No Name]".to_string();
        if let Some(name) = &self.document.file_name {
            file_name = name.clone();
            file_name.truncate(20);
        }
        status = format!(
            "{} - {} lines{}",
            file_name,
            self.document.len(),
            modified_indicator
        );

        let line_indicator = format!(
            "{} | {}/{}",
            self.document.file_type(),
            self.cursor_position.y.saturating_add(1),
            self.document.len()
        );
        #[allow(clippy::integer_arithmetic)]
        let len = status.len() + line_indicator.len();
        status.push_str(&" ".repeat(width.saturating_sub(len)));
        status = format!("{}{}", status, line_indicator);
        status.truncate(width);
        Terminal::set_bg_color(STATUS_BG_COLOR);
        Terminal::set_fg_color(STATUS_FG_COLOR);
        println!("{}\r", status);
        Terminal::reset_fg_color();
        Terminal::reset_bg_color();
    }

    fn draw_message_bar(&self) {
        Terminal::clear_current_line();
        let message = &self.status_message;
        if Instant::now() - message.time < Duration::new(5, 0) {
            let mut text = message.text.clone();
            text.truncate(self.terminal.size().width as usize);
            print!("{}", text);
        }
    }
    fn prompt<C>(&mut self, prompt: &str, mut callback: C) -> Result<Option<String>, std::io::Error>
where
    C: FnMut(&mut Self, KeyEvent, &String),
{
    let mut result = String::new();
    loop {
        self.status_message = StatusMessage::from(format!("{}{}", prompt, result));
        self.refresh_screen()?;
        let key = Terminal::read_key()?;
        match key {
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => result.truncate(result.len().saturating_sub(1)),
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => break,
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
            } => {
                if !c.is_control() {
                    result.push(c);
                }
            }
            KeyEvent {
                code: KeyCode::Esc,
                ..
            } => {
                result.truncate(0);
                break;
            }
            _ => (),
        }
        callback(self, key, &result);
    }
    self.status_message = StatusMessage::from(String::new());
    if result.is_empty() {
        return Ok(None);
    }
    Ok(Some(result))
}
}
fn die(e: std::io::Error) {
    Terminal::clear_screen();
    std::panic::panic_any(e);
}
