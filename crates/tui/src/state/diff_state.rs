use super::app_state::DiffState;

impl DiffState {
    pub(crate) fn invalidate_diff_preview_cache(&mut self) {
        self.diff_preview_cache_key = None;
        self.diff_preview_cache_hash = 0;
    }

    pub(crate) fn clamp_selected_diff_index(&mut self, len: usize) {
        self.selected_diff_index = if len == 0 {
            0
        } else {
            self.selected_diff_index.min(len.saturating_sub(1))
        };
    }
}
