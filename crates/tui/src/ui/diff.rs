use ratatui::{Frame, layout::Rect};

use super::components::{
    UiComponent, diff_list::DiffList, diff_preview::DiffPreview, diff_repo_bar::DiffRepoBar,
};
use crate::{diff::diff_rows_with_all_filtered, state::AppState};

pub(crate) fn render_diff_pane(f: &mut Frame, app: &AppState, area: Rect) {
    let sections = crate::layout::split_diff_pane(area);

    <DiffRepoBar as UiComponent>::render(f, app, sections.repo_bar);
    <DiffList as UiComponent>::render(f, app, sections.files);
    <DiffPreview as UiComponent>::render(f, app, sections.preview);
}

fn repo_name_from_path(path: &str) -> Option<&str> {
    let first = path.split('/').next()?;
    if first.is_empty() { None } else { Some(first) }
}

fn selected_repo_status_from_diff(app: &AppState) -> Option<usize> {
    if app.diff.repo_statuses.is_empty() {
        return None;
    }
    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
    let selected = rows.get(app.diff.selected_diff_index)?;
    let path = selected
        .new_path
        .as_deref()
        .or(selected.old_path.as_deref())
        .unwrap_or(&selected.key);
    let repo = repo_name_from_path(path)?;
    app.diff
        .repo_statuses
        .iter()
        .position(|r| r.repo_name == repo)
}

pub(crate) fn sync_selected_repo_from_diff_selection(app: &mut AppState) {
    if let Some(idx) = selected_repo_status_from_diff(app) {
        crate::selection_hooks::set_selected_repo_index(app, idx);
    }
}
