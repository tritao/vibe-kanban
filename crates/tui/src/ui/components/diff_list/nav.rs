use crate::{
    state::{AppState, CommitEntry},
    ui::list_nav,
};

pub(super) fn select_diff_file(app: &mut AppState, idx: usize) -> bool {
    let rows = crate::store::diff::DiffStore::new(app.diff.diff_store.as_value())
        .rows_with_all_filtered(app.diff.diff_show_untracked);
    if !list_nav::select_index(&mut app.diff.selected_diff_index, idx, rows.len()) {
        return false;
    }
    crate::selection::change::on_diff_file_selected(app);
    true
}

pub(super) fn select_adjacent_diff_file(app: &mut AppState, delta: i32) -> bool {
    let rows = crate::store::diff::DiffStore::new(app.diff.diff_store.as_value())
        .rows_with_all_filtered(app.diff.diff_show_untracked);
    if !list_nav::select_delta(&mut app.diff.selected_diff_index, delta, rows.len()) {
        return false;
    }
    crate::selection::change::on_diff_file_selected(app);
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
        .map(|v| v.items.as_slice())
        .unwrap_or(&[])
}

pub(super) fn select_commit(app: &mut AppState, idx: usize) -> bool {
    let len = commits_for_selected_repo(app).len();
    if !list_nav::select_index(&mut app.diff.selected_commit_index, idx, len) {
        return false;
    }
    crate::selection::change::on_commit_selected(app);
    true
}

pub(super) fn select_adjacent_commit(app: &mut AppState, delta: i32) -> bool {
    let len = commits_for_selected_repo(app).len();
    if !list_nav::select_delta(&mut app.diff.selected_commit_index, delta, len) {
        return false;
    }
    crate::selection::change::on_commit_selected(app);
    true
}
