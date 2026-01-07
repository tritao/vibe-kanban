use crossterm::event::MouseEvent;

use super::{
    super::{focus, sel},
    hit_test::{commit_list_hit_at, diff_files_hit_at},
};
use crate::{
    layout::rect_contains,
    state::AppState,
    ui::components::{UiComponent, diff_repo_bar::DiffRepoBar},
};

pub(super) fn handle_diff_left_click(
    app: &mut AppState,
    mouse: MouseEvent,
    layout: &crate::layout::MainLayoutRects,
) -> bool {
    let col = mouse.column;
    let row = mouse.row;

    if rect_contains(layout.diff_repo_bar, col, row) {
        focus::focus_diff(app);
        if let Some(evt) =
            <DiffRepoBar as UiComponent>::hit_test(app, layout.diff_repo_bar, col, row)
        {
            let _ = <DiffRepoBar as UiComponent>::on_event(app, evt);
        }
        return true;
    }
    if rect_contains(layout.diff_files, col, row) {
        focus::focus_diff_files(app);
        match app.diff.list_mode {
            crate::state::DiffListMode::Files => {
                if let Some(idx) = diff_files_hit_at(app, layout.diff_files, col, row) {
                    sel::select_diff_file(app, idx);
                }
            }
            crate::state::DiffListMode::Commits => {
                if let Some(idx) = commit_list_hit_at(app, layout.diff_files, col, row) {
                    sel::select_commit(app, idx);
                }
            }
        }
        return true;
    }
    if rect_contains(layout.diff_preview, col, row) {
        focus::focus_diff_preview(app);
        return true;
    }

    false
}
