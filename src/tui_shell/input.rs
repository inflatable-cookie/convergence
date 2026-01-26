#[derive(Debug, Default)]
pub(super) struct Input {
    pub(super) buf: String,
    pub(super) cursor: usize,
    pub(super) history: Vec<String>,
    pub(super) history_pos: Option<usize>,
}

impl Input {
    pub(super) fn clear(&mut self) {
        self.buf.clear();
        self.cursor = 0;
        self.history_pos = None;
    }

    pub(super) fn insert_char(&mut self, c: char) {
        self.buf.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub(super) fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        self.buf.remove(self.cursor);
    }

    pub(super) fn delete(&mut self) {
        if self.cursor >= self.buf.len() {
            return;
        }
        self.buf.remove(self.cursor);
    }

    pub(super) fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub(super) fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.buf.len());
    }

    pub(super) fn set(&mut self, s: String) {
        self.buf = s;
        self.cursor = self.buf.len();
    }

    pub(super) fn push_history(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        if self.history.last().map(|s| s.as_str()) == Some(line) {
            return;
        }
        self.history.push(line.to_string());
        self.history_pos = None;
    }

    pub(super) fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let next = match self.history_pos {
            None => self.history.len().saturating_sub(1),
            Some(i) => i.saturating_sub(1),
        };
        self.history_pos = Some(next);
        self.set(self.history[next].clone());
    }

    pub(super) fn history_down(&mut self) {
        let Some(i) = self.history_pos else {
            return;
        };
        if i + 1 >= self.history.len() {
            self.history_pos = None;
            self.clear();
            return;
        }
        let next = i + 1;
        self.history_pos = Some(next);
        self.set(self.history[next].clone());
    }
}
