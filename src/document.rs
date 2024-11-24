use crate::FileType;
use crate::Position;
use crate::Row;
use crate::SearchDirection;
use std::fs;
use std::io::{Error, Write};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct EditorState {
    rows: Vec<Row>,
    cursor_position: Position,
}

pub struct Document {
    rows: Vec<Row>,
    pub file_name: Option<String>,
    dirty: bool,
    file_type: FileType,
    history: Vec<(Vec<Row>, Position)>,
    history_index: usize,
    last_edit_time: Option<Instant>,
    batch_duration: Duration,
}

impl Document {
    pub fn open(filename: &str) -> Result<Self, std::io::Error> {
        let contents = fs::read_to_string(filename)?;
        let file_type = FileType::from(filename);
        let mut rows = Vec::new();
        for value in contents.lines() {
            rows.push(Row::from(value));
        }
        Ok(Self {
            rows,
            file_name: Some(filename.to_string()),
            dirty: false,
            file_type,
            history: vec![(Vec::new(), Position::default())],
            history_index: 0,
            last_edit_time: None,
            batch_duration: Duration::from_millis(1000),
        })
    }
    pub fn file_type(&self) -> String {
        self.file_type.name()
    }
    pub fn row(&self, index: usize) -> Option<&Row> {
        self.rows.get(index)
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    fn insert_newline(&mut self, at: &Position) {
        if at.y > self.rows.len() {
            return;
        }
        if at.y == self.rows.len() {
            self.rows.push(Row::default());
            return;
        }
        #[allow(clippy::indexing_slicing)]
        let current_row = &mut self.rows[at.y];
        let new_row = current_row.split(at.x);
        #[allow(clippy::integer_arithmetic)]
        self.rows.insert(at.y + 1, new_row);
    }
    pub fn insert(&mut self, at: &Position, c: char) {
        self.save_state(&Position::default());
        if at.y > self.rows.len() {
            return;
        }
        self.dirty = true;
        if c == '\n' {
            self.insert_newline(at);
        } else if at.y == self.rows.len() {
            let mut row = Row::default();
            row.insert(0, c);
            self.rows.push(row);
        } else {
            #[allow(clippy::indexing_slicing)]
            let row = &mut self.rows[at.y];
            row.insert(at.x, c);
        }
        self.unhighlight_rows(at.y);
    }

    fn unhighlight_rows(&mut self, start: usize) {
        let start = start.saturating_sub(1);
        for row in self.rows.iter_mut().skip(start) {
            row.is_highlighted = false;
        }
    }
    #[allow(clippy::integer_arithmetic, clippy::indexing_slicing)]
    pub fn delete(&mut self, at: &Position) -> bool {
        self.save_state(&Position::default());
        let len = self.rows.len();
        if at.y >= len {
            return false;
        }
        self.dirty = true;
        if at.x == self.rows[at.y].len() && at.y + 1 < len {
            let next_row = self.rows.remove(at.y + 1);
            let row = &mut self.rows[at.y];
            row.append(&next_row);
        } else {
            let row = &mut self.rows[at.y];
            row.delete(at.x);
        }
        self.unhighlight_rows(at.y);
        true
    }
    pub fn save(&mut self) -> Result<(), Error> {
        if let Some(file_name) = &self.file_name {
            let mut file = fs::File::create(file_name)?;
            self.file_type = FileType::from(file_name);
            for row in &mut self.rows {
                file.write_all(row.as_bytes())?;
                file.write_all(b"\n")?;
            }
            self.dirty = false;
        }
        Ok(())
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    #[allow(clippy::indexing_slicing)]
    pub fn find(&self, query: &str, at: &Position, direction: SearchDirection) -> Option<Position> {
        if at.y >= self.rows.len() {
            return None;
        }
        let mut position = Position { x: at.x, y: at.y };

        let start = if direction == SearchDirection::Forward {
            at.y
        } else {
            0
        };
        let end = if direction == SearchDirection::Forward {
            self.rows.len()
        } else {
            at.y.saturating_add(1)
        };
        for _ in start..end {
            if let Some(row) = self.rows.get(position.y) {
                if let Some(x) = row.find(&query, position.x, direction) {
                    position.x = x;
                    return Some(position);
                }
                if direction == SearchDirection::Forward {
                    position.y = position.y.saturating_add(1);
                    position.x = 0;
                } else {
                    position.y = position.y.saturating_sub(1);
                    position.x = self.rows[position.y].len();
                }
            } else {
                return None;
            }
        }
        None
    }
    pub fn highlight(&mut self, word: &Option<String>, until: Option<usize>) {
        let mut start_with_comment = false;
        let until = if let Some(until) = until {
            if until.saturating_add(1) < self.rows.len() {
                until.saturating_add(1)
            } else {
                self.rows.len()
            }
        } else {
            self.rows.len()
        };
        #[allow(clippy::indexing_slicing)]
        for row in &mut self.rows[..until] {
            start_with_comment = row.highlight(
                &self.file_type.highlighting_options(),
                word,
                start_with_comment,
            );
        }
    }
    fn should_create_new_state(&mut self) -> bool {
        if let Some(last_time) = self.last_edit_time {
            if last_time.elapsed() > self.batch_duration {
                self.last_edit_time = Some(Instant::now());
                true
            } else {
                false
            }
        } else {
            self.last_edit_time = Some(Instant::now());
            true
        }
    }
    fn save_state(&mut self, cursor_position: &Position) {
        if self.should_create_new_state() {
            self.history.truncate(self.history_index + 1);
            self.history.push((self.rows.clone(), cursor_position.clone()));
            self.history_index = self.history.len() - 1;
        } else {
            self.history[self.history_index] = (self.rows.clone(), cursor_position.clone());
        }
    }
    pub fn undo(&mut self) -> Option<Position> {
        if self.history_index > 0 {
            self.history_index -= 1;
            let (rows, position) = &self.history[self.history_index];
            self.rows = rows.clone();
            self.last_edit_time = None;
            Some(position.clone())
        } else {
            None
        }
    }
    pub fn redo(&mut self) -> Option<Position> {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let (rows, position) = &self.history[self.history_index];
            self.rows = rows.clone();
            self.last_edit_time = None;
            Some(position.clone())
        } else {
            None
        }
    }
    pub fn default() -> Self {
        Self {
            rows: Vec::new(),
            file_name: None,
            dirty: false,
            file_type: FileType::default(),
            history: vec![(Vec::new(), Position::default())],
            history_index: 0,
            last_edit_time: None,
            batch_duration: Duration::from_millis(1000),
        }
    }
}
