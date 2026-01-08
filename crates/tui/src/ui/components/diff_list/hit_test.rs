use ratatui::layout::Rect;

use crate::{state::AppState, ui::list_nav};

pub(super) fn diff_file_index(app: &AppState, area: Rect, row: u16) -> Option<usize> {
    let rows = crate::store::diff::DiffStore::new(app.diff.diff_store.as_value())
        .rows_with_all_filtered(app.diff.diff_show_untracked);
    list_nav::index_at_row(area, row, app.diff.selected_diff_index, rows.len(), 0)
}

pub(super) fn commit_index(app: &AppState, area: Rect, row: u16) -> Option<usize> {
    let commits = super::nav::commits_for_selected_repo(app);
    let footer = super::commit_footer(app);
    let footer_rows = usize::from(footer.loading || footer.has_more);

    list_nav::index_at_row(
        area,
        row,
        app.diff.selected_commit_index,
        commits.len(),
        footer_rows,
    )
}
