use ratatui::layout::Rect;

use crate::{diff::diff_rows_with_all_filtered, state::AppState, util::window_for_list};

pub(super) fn diff_file_index(app: &AppState, area: Rect, row: u16) -> Option<usize> {
    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
    if rows.is_empty() {
        return None;
    }

    let inner_y0 = area.y.saturating_add(1);
    let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
    if row < inner_y0 || row >= inner_y1 {
        return None;
    }

    let height = area.height.saturating_sub(2) as usize;
    if height == 0 {
        return None;
    }

    let selected = app
        .diff
        .selected_diff_index
        .min(rows.len().saturating_sub(1));
    let (start, end, _) = window_for_list(rows.len(), selected, height);
    let visible_len = end.saturating_sub(start);

    let inner_row = row.saturating_sub(inner_y0) as usize;
    if inner_row >= visible_len {
        return None;
    }
    Some(start + inner_row)
}

pub(super) fn commit_index(app: &AppState, area: Rect, row: u16) -> Option<usize> {
    let commits = super::nav::commits_for_selected_repo(app);
    if commits.is_empty() {
        return None;
    }

    let inner_y0 = area.y.saturating_add(1);
    let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
    if row < inner_y0 || row >= inner_y1 {
        return None;
    }

    let height = area.height.saturating_sub(2) as usize;
    if height == 0 {
        return None;
    }

    let selected = app
        .diff
        .selected_commit_index
        .min(commits.len().saturating_sub(1));
    let (start, end, _) = window_for_list(commits.len(), selected, height);
    let visible_len = end.saturating_sub(start);

    let inner_row = row.saturating_sub(inner_y0) as usize;
    if inner_row >= visible_len {
        return None;
    }
    Some(start + inner_row)
}
