use ratatui::layout::Rect;

use crate::{diff::diff_rows_with_all_filtered, state::AppState, ui::list_nav};

pub(super) fn diff_file_index(app: &AppState, area: Rect, row: u16) -> Option<usize> {
    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
    list_nav::index_at_row(area, row, app.diff.selected_diff_index, rows.len(), 0)
}

pub(super) fn commit_index(app: &AppState, area: Rect, row: u16) -> Option<usize> {
    let commits = super::nav::commits_for_selected_repo(app);
    let footer_rows = {
        let repo_id = app
            .diff
            .repo_statuses
            .get(app.diff.selected_repo_index)
            .map(|r| r.repo_id);
        let loading = repo_id
            .and_then(|id| app.diff.commits_loading_by_repo.get(&id))
            .map(|i| i.visible)
            .unwrap_or(false);
        let has_more = repo_id
            .and_then(|id| app.diff.commits_has_more_by_repo.get(&id).copied())
            .unwrap_or(false);
        usize::from(loading || has_more)
    };

    list_nav::index_at_row(
        area,
        row,
        app.diff.selected_commit_index,
        commits.len(),
        footer_rows,
    )
}
