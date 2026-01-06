use std::time::Duration;

use ratatui::text::Line;

use crate::{
    diff::diff_rows_with_all_filtered,
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

    app.diff.list_mode = crate::state::DiffListMode::Files;
    app.diff.selected_commit_index = 0;
    app.diff.commit_preview_lines = vec![Line::from("No commit selected")];
    app.diff.commit_preview_loading = false;
}

pub(in crate::actions) fn select_adjacent_diff_file(app: &mut AppState, delta: i32) {
    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
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
    crate::commands::request_stack_status_refresh(app);
    crate::commands::request_commit_list_refresh(app);
    schedule_diff_preview_refresh(app, Duration::from_millis(0));
}

pub(in crate::actions) fn select_adjacent_commit(app: &mut AppState, delta: i32) {
    let Some(repo) = app.diff.repo_statuses.get(app.diff.selected_repo_index) else {
        app.diff.selected_commit_index = 0;
        return;
    };
    let commits = app
        .diff
        .commits_by_repo
        .get(&repo.repo_id)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    if commits.is_empty() {
        app.diff.selected_commit_index = 0;
        return;
    }

    let cur = app.diff.selected_commit_index.min(commits.len() - 1);
    let next = crate::selection::clamp_index(cur, delta, commits.len());
    if next == cur {
        return;
    }
    app.diff.selected_commit_index = next;
    app.diff.diff_scroll_offset = 0;
    crate::commands::request_commit_preview_refresh(app);
}

pub(in crate::actions) fn select_commit(app: &mut AppState, idx: usize) {
    let Some(repo) = app.diff.repo_statuses.get(app.diff.selected_repo_index) else {
        app.diff.selected_commit_index = 0;
        return;
    };
    let commits = app
        .diff
        .commits_by_repo
        .get(&repo.repo_id)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    if commits.is_empty() {
        app.diff.selected_commit_index = 0;
        return;
    }

    let next = idx.min(commits.len() - 1);
    if next == app.diff.selected_commit_index {
        return;
    }
    app.diff.selected_commit_index = next;
    app.diff.diff_scroll_offset = 0;
    crate::commands::request_commit_preview_refresh(app);
}

pub(in crate::actions) fn select_diff_file(app: &mut AppState, idx: usize) {
    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
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
    crate::commands::request_stack_status_refresh(app);
    crate::commands::request_commit_list_refresh(app);
    schedule_diff_preview_refresh(app, Duration::from_millis(0));
}
