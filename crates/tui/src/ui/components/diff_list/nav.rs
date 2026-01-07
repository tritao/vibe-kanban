use crate::{
    diff::diff_rows_with_all_filtered,
    state::{AppState, CommitEntry},
};

pub(super) fn select_diff_file(app: &mut AppState, idx: usize) -> bool {
    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
    if rows.is_empty() {
        app.diff.selected_diff_index = 0;
        return false;
    }
    let next = idx.min(rows.len().saturating_sub(1));
    if next == app.diff.selected_diff_index {
        return false;
    }
    app.diff.selected_diff_index = next;
    crate::selection_hooks::on_diff_file_selected(app);
    true
}

pub(super) fn select_adjacent_diff_file(app: &mut AppState, delta: i32) -> bool {
    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
    if rows.is_empty() {
        app.diff.selected_diff_index = 0;
        return false;
    }
    let cur = app
        .diff
        .selected_diff_index
        .min(rows.len().saturating_sub(1));
    let next = crate::selection::clamp_index(cur, delta, rows.len());
    if next == cur {
        return false;
    }
    app.diff.selected_diff_index = next;
    crate::selection_hooks::on_diff_file_selected(app);
    true
}

pub(super) fn commits_for_selected_repo<'a>(app: &'a AppState) -> &'a [CommitEntry] {
    let repo_id = app
        .diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .map(|r| r.repo_id);
    repo_id
        .and_then(|id| app.diff.commits_by_repo.get(&id))
        .map(|v| v.as_slice())
        .unwrap_or(&[])
}

pub(super) fn select_commit(app: &mut AppState, idx: usize) -> bool {
    let commits = commits_for_selected_repo(app);
    if commits.is_empty() {
        app.diff.selected_commit_index = 0;
        return false;
    }
    let next = idx.min(commits.len().saturating_sub(1));
    if next == app.diff.selected_commit_index {
        return false;
    }
    app.diff.selected_commit_index = next;
    crate::selection_hooks::on_commit_selected(app);
    true
}

pub(super) fn select_adjacent_commit(app: &mut AppState, delta: i32) -> bool {
    let commits = commits_for_selected_repo(app);
    if commits.is_empty() {
        app.diff.selected_commit_index = 0;
        return false;
    }
    let cur = app
        .diff
        .selected_commit_index
        .min(commits.len().saturating_sub(1));
    let next = crate::selection::clamp_index(cur, delta, commits.len());
    if next == cur {
        return false;
    }
    app.diff.selected_commit_index = next;
    crate::selection_hooks::on_commit_selected(app);
    true
}
