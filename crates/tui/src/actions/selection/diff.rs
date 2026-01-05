use std::time::Duration;

use ratatui::text::Line;

use crate::{
    diff::diff_rows_with_all,
    diff_preview::{cancel_diff_preview_job, schedule_diff_preview_refresh},
    state::AppState,
    ui::sync_selected_repo_from_diff_selection,
};

pub(in crate::actions) fn reset_diff_stream_state(app: &mut AppState) {
    app.diff.diff_store = serde_json::json!({ "entries": {} });
    app.diff.selected_diff_index = 0;
    app.diff.diff_scroll_offset = 0;
    app.diff.diff_preview_cache_key = None;
    app.diff.diff_preview_cache_hash = 0;
    app.diff.diff_preview_cache_width = 0;
    app.diff.diff_preview_lines = vec![Line::from("No diffs")];
    app.diff.diff_preview_pending = false;
    app.diff.diff_preview_next_refresh_at = None;
    cancel_diff_preview_job(app);
}

pub(in crate::actions) fn select_adjacent_diff_file(app: &mut AppState, delta: i32) {
    let rows = diff_rows_with_all(&app.diff.diff_store);
    if rows.is_empty() {
        app.diff.selected_diff_index = 0;
        return;
    }

    let cur = app.diff.selected_diff_index.min(rows.len() - 1);
    let next = crate::selection::clamp_index(cur, delta, rows.len());
    if next == cur {
        return;
    }

    app.diff.selected_diff_index = next;
    app.diff.diff_scroll_offset = 0;
    sync_selected_repo_from_diff_selection(app);
    schedule_diff_preview_refresh(app, Duration::from_millis(0));
}

pub(in crate::actions) fn select_diff_file(app: &mut AppState, idx: usize) {
    let rows = diff_rows_with_all(&app.diff.diff_store);
    if rows.is_empty() {
        app.diff.selected_diff_index = 0;
        return;
    }

    let next = idx.min(rows.len().saturating_sub(1));
    if next == app.diff.selected_diff_index {
        return;
    }

    app.diff.selected_diff_index = next;
    app.diff.diff_scroll_offset = 0;
    sync_selected_repo_from_diff_selection(app);
    schedule_diff_preview_refresh(app, Duration::from_millis(0));
}
