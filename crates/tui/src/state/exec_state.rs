use super::app_state::ExecState;

impl ExecState {
    pub(crate) fn clear_log_selection(&mut self) -> bool {
        let mut dirty = false;
        dirty |= self.log_selected.take().is_some();
        dirty |= self.log_mouse_selecting;
        dirty |= self.log_mouse_select_anchor.take().is_some();
        dirty |= self.log_mouse_select_range.take().is_some();
        self.log_mouse_selecting = false;
        dirty
    }

    pub(crate) fn start_log_mouse_selection(&mut self, line_idx: usize) {
        self.log_mouse_selecting = true;
        self.log_mouse_select_anchor = Some(line_idx);
        self.log_mouse_select_range = Some((line_idx, line_idx));
    }

    pub(crate) fn update_log_mouse_selection(&mut self, cur: usize) -> bool {
        let Some(anchor) = self.log_mouse_select_anchor else {
            return false;
        };
        let (a, b) = if anchor <= cur {
            (anchor, cur)
        } else {
            (cur, anchor)
        };
        self.log_mouse_select_range = Some((a, b));
        true
    }

    pub(crate) fn finish_log_mouse_selection(&mut self) -> bool {
        let dirty = self.log_mouse_selecting;
        self.log_mouse_selecting = false;
        dirty
    }
}
