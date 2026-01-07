#[derive(Debug, Clone, Default)]
pub(crate) struct TextFieldState {
    pub(crate) buffer: String,
    pub(crate) cursor: usize, // byte index
    pub(crate) goal_col: Option<usize>,
    pub(crate) scroll_x: u16, // columns
    pub(crate) scroll_y: u16, // lines
    undo: Vec<TextFieldSnapshot>,
    redo: Vec<TextFieldSnapshot>,
}

#[derive(Debug, Clone)]
struct TextFieldSnapshot {
    buffer: String,
    cursor: usize,
    goal_col: Option<usize>,
    scroll_x: u16,
    scroll_y: u16,
}

impl TextFieldState {
    fn snapshot(&self) -> TextFieldSnapshot {
        TextFieldSnapshot {
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            goal_col: self.goal_col,
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
        }
    }

    fn restore(&mut self, snap: TextFieldSnapshot) {
        self.buffer = snap.buffer;
        self.cursor = snap.cursor;
        self.goal_col = snap.goal_col;
        self.scroll_x = snap.scroll_x;
        self.scroll_y = snap.scroll_y;
        self.clamp_cursor();
    }

    fn push_undo(&mut self) {
        const MAX_UNDO: usize = 200;
        self.undo.push(self.snapshot());
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub(crate) fn undo(&mut self) -> bool {
        let Some(snap) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(snap);
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some(snap) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(snap);
        true
    }

    pub(crate) fn clamp_cursor(&mut self) {
        self.cursor = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
    }

    pub(crate) fn set_end(&mut self) {
        self.cursor = self.buffer.len();
        self.goal_col = None;
    }

    pub(crate) fn clear(&mut self) {
        if !self.buffer.is_empty() {
            self.push_undo();
        }
        self.buffer.clear();
        self.cursor = 0;
        self.goal_col = None;
        self.scroll_x = 0;
        self.scroll_y = 0;
    }

    pub(crate) fn move_left(&mut self) {
        self.cursor = crate::text::edit::prev_cursor(&self.buffer, self.cursor);
        self.goal_col = None;
    }

    pub(crate) fn move_right(&mut self) {
        self.cursor = crate::text::edit::next_cursor(&self.buffer, self.cursor);
        self.goal_col = None;
    }

    pub(crate) fn move_word_left(&mut self) {
        self.cursor = crate::text::edit::prev_word_cursor(&self.buffer, self.cursor);
        self.goal_col = None;
    }

    pub(crate) fn move_word_right(&mut self) {
        self.cursor = crate::text::edit::next_word_cursor(&self.buffer, self.cursor);
        self.goal_col = None;
    }

    pub(crate) fn move_home(&mut self, multiline: bool) {
        if !multiline {
            self.cursor = 0;
            self.goal_col = None;
            return;
        }
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let ranges = crate::text::edit::line_ranges(&self.buffer);
        let line = crate::text::edit::cursor_line_index(&ranges, cur);
        self.cursor = ranges.get(line).map(|(s, _)| *s).unwrap_or(0);
        self.goal_col = None;
    }

    pub(crate) fn move_end(&mut self, multiline: bool) {
        if !multiline {
            self.cursor = self.buffer.len();
            self.goal_col = None;
            return;
        }
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let ranges = crate::text::edit::line_ranges(&self.buffer);
        let line = crate::text::edit::cursor_line_index(&ranges, cur);
        self.cursor = ranges
            .get(line)
            .map(|(_, e)| *e)
            .unwrap_or(self.buffer.len());
        self.goal_col = None;
    }

    pub(crate) fn move_up(&mut self) {
        let (next, goal) =
            crate::text::edit::move_cursor_vertically(&self.buffer, self.cursor, -1, self.goal_col);
        self.cursor = next;
        self.goal_col = goal;
    }

    pub(crate) fn move_down(&mut self) {
        let (next, goal) =
            crate::text::edit::move_cursor_vertically(&self.buffer, self.cursor, 1, self.goal_col);
        self.cursor = next;
        self.goal_col = goal;
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        self.push_undo();
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        self.buffer.insert(cur, ch);
        self.cursor = cur + ch.len_utf8();
        self.goal_col = None;
    }

    pub(crate) fn backspace(&mut self) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let prev = crate::text::edit::prev_cursor(&self.buffer, cur);
        if prev < cur {
            self.push_undo();
            self.buffer.replace_range(prev..cur, "");
            self.cursor = prev;
        } else {
            self.cursor = 0;
        }
        self.goal_col = None;
    }

    pub(crate) fn backspace_word(&mut self) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let prev = crate::text::edit::prev_word_cursor(&self.buffer, cur);
        if prev < cur {
            self.push_undo();
            self.buffer.replace_range(prev..cur, "");
            self.cursor = prev;
        } else {
            self.cursor = 0;
        }
        self.goal_col = None;
    }

    pub(crate) fn delete(&mut self) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let next = crate::text::edit::next_cursor(&self.buffer, cur);
        if cur < next {
            self.push_undo();
            self.buffer.replace_range(cur..next, "");
            self.cursor = cur;
        }
        self.goal_col = None;
    }

    pub(crate) fn delete_word(&mut self) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let next = crate::text::edit::next_word_cursor(&self.buffer, cur);
        if cur < next {
            self.push_undo();
            self.buffer.replace_range(cur..next, "");
            self.cursor = cur;
        }
        self.goal_col = None;
    }

    #[allow(dead_code)]
    pub(crate) fn delete_to_end_of_line(&mut self) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let ranges = crate::text::edit::line_ranges(&self.buffer);
        let line = crate::text::edit::cursor_line_index(&ranges, cur);
        let line_end = ranges.get(line).map(|(_, e)| *e).unwrap_or(cur);
        if cur < line_end {
            self.push_undo();
            self.buffer.replace_range(cur..line_end, "");
            self.cursor = cur;
        }
        self.goal_col = None;
    }

    #[allow(dead_code)]
    pub(crate) fn insert_newline(&mut self) {
        self.push_undo();
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        self.buffer.insert(cur, '\n');
        self.cursor = cur + 1;
        self.goal_col = None;
    }

    #[allow(dead_code)]
    pub(crate) fn set_buffer(&mut self, text: String) {
        self.buffer = text;
        self.cursor = self.buffer.len();
        self.goal_col = None;
        self.scroll_x = 0;
        self.scroll_y = 0;
        self.undo.clear();
        self.redo.clear();
        self.clamp_cursor();
    }

    #[allow(dead_code)]
    pub(crate) fn render_lines(&self) -> Vec<String> {
        self.buffer.split('\n').map(|s| s.to_string()).collect()
    }

    pub(crate) fn cursor_line_col(&self) -> (usize, usize) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let ranges = crate::text::edit::line_ranges(&self.buffer);
        let line = crate::text::edit::cursor_line_index(&ranges, cur);
        let (line_start, line_end) = ranges.get(line).copied().unwrap_or((0, 0));
        let col =
            crate::text::edit::cursor_col_in_line(&self.buffer, line_start, cur.min(line_end));
        (line, col)
    }

    pub(crate) fn ensured_scroll(&self, inner_w: usize, inner_h: usize) -> (u16, u16) {
        let (line, col) = self.cursor_line_col();

        let inner_h = inner_h.max(1) as i64;
        let inner_w = inner_w.max(1) as i64;
        let cy = line as i64;
        let cx = col as i64;
        let sy = self.scroll_y as i64;
        let sx = self.scroll_x as i64;

        let mut new_sy = sy;
        if cy < sy {
            new_sy = cy;
        } else if cy >= sy + inner_h {
            new_sy = cy - inner_h + 1;
        }

        let mut new_sx = sx;
        if cx < sx {
            new_sx = cx;
        } else if cx >= sx + inner_w {
            new_sx = cx - inner_w + 1;
        }

        (new_sy.max(0) as u16, new_sx.max(0) as u16)
    }

    pub(crate) fn ensure_cursor_visible(&mut self, inner_w: usize, inner_h: usize) {
        let (y, x) = self.ensured_scroll(inner_w, inner_h);
        self.scroll_y = y;
        self.scroll_x = x;
    }
}
