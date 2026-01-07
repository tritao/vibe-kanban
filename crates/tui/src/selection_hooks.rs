use std::time::Duration;

use crate::{
    diff_preview::schedule_diff_preview_refresh,
    state::{AppState, DiffListMode},
};

pub(crate) fn set_selected_repo_index(app: &mut AppState, idx: usize) {
    if app.diff.repo_statuses.is_empty() {
        app.diff.selected_repo_index = 0;
        return;
    }
    let idx = idx.min(app.diff.repo_statuses.len().saturating_sub(1));
    if app.diff.selected_repo_index == idx {
        return;
    }
    app.diff.selected_repo_index = idx;
    on_repo_selected(app);
}

pub(crate) fn on_repo_selected(app: &mut AppState) {
    crate::commands::request_stack_status_refresh(app);
    if app.diff.list_mode == DiffListMode::Commits {
        crate::commands::request_commit_list_refresh(app);
    }
    // Repo selection can affect the diff preview (e.g. combined repo filter), and this is also a
    // good time to refresh the preview if it was stale.
    schedule_diff_preview_refresh(app, Duration::from_millis(0));
}

pub(crate) fn on_diff_file_selected(app: &mut AppState) {
    app.diff.diff_scroll_offset = 0;
    crate::ui::sync_selected_repo_from_diff_selection(app);
    // Selecting a different file should always refresh the preview, even if the repo selection
    // didn't change.
    schedule_diff_preview_refresh(app, Duration::from_millis(0));
}

pub(crate) fn on_commit_selected(app: &mut AppState) {
    app.diff.diff_scroll_offset = 0;
    crate::commands::request_commit_preview_refresh(app);
}
