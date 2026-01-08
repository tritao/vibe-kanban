#[derive(Debug, Clone, Copy)]
pub(crate) struct ScrollFromTop {
    pub(crate) offset: usize,
}

impl ScrollFromTop {
    #[allow(dead_code)]
    pub(crate) fn new(offset: usize) -> Self {
        Self { offset }
    }

    pub(crate) fn normalize(&mut self, len: usize, visible: usize) {
        self.offset = crate::ui::scroll::clamp_offset(self.offset, len, visible);
    }

    pub(crate) fn scroll_up(&mut self, len: usize, visible: usize, lines: usize) -> bool {
        let before = self.offset;
        self.offset = self.offset.saturating_sub(lines);
        self.normalize(len, visible);
        self.offset != before
    }

    pub(crate) fn scroll_down(&mut self, len: usize, visible: usize, lines: usize) -> bool {
        let before = self.offset;
        self.offset = self.offset.saturating_add(lines);
        self.normalize(len, visible);
        self.offset != before
    }

    pub(crate) fn visible_range(&self, len: usize, visible: usize) -> (usize, usize) {
        crate::ui::scroll::visible_window_from_top(self.offset, len, visible)
    }
}

impl Default for ScrollFromTop {
    fn default() -> Self {
        Self { offset: 0 }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ScrollFromEnd {
    pub(crate) autoscroll: bool,
    pub(crate) offset_from_end: usize,
}

impl ScrollFromEnd {
    #[allow(dead_code)]
    pub(crate) fn new(autoscroll: bool, offset_from_end: usize) -> Self {
        Self {
            autoscroll,
            offset_from_end,
        }
    }

    pub(crate) fn normalize(&mut self, len: usize, visible: usize) {
        self.offset_from_end = crate::ui::scroll::clamp_offset(self.offset_from_end, len, visible);
        if self.autoscroll {
            self.offset_from_end = 0;
        }
        if self.offset_from_end == 0 {
            self.autoscroll = true;
        }
    }

    pub(crate) fn scroll_older(&mut self, len: usize, visible: usize, lines: usize) {
        self.autoscroll = false;
        self.offset_from_end = self.offset_from_end.saturating_add(lines);
        self.normalize(len, visible);
    }

    pub(crate) fn scroll_newer(&mut self, len: usize, visible: usize, lines: usize) {
        self.offset_from_end = self.offset_from_end.saturating_sub(lines);
        self.normalize(len, visible);
    }

    pub(crate) fn scroll_to_end(&mut self) {
        self.autoscroll = true;
        self.offset_from_end = 0;
    }

    pub(crate) fn visible_range(&self, len: usize, visible: usize) -> (usize, usize) {
        let offset = if self.autoscroll {
            0
        } else {
            self.offset_from_end
        };
        crate::ui::scroll::visible_window_from_end(offset, len, visible)
    }
}

impl Default for ScrollFromEnd {
    fn default() -> Self {
        Self {
            autoscroll: true,
            offset_from_end: 0,
        }
    }
}
