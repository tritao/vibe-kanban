use crossterm::event::MouseEvent;

use super::super::focus;
use crate::{
    layout::rect_contains,
    state::AppState,
    ui::components::{UiComponent, diff_list::DiffList, diff_repo_bar::DiffRepoBar},
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
        if let Some(evt) = <DiffList as UiComponent>::hit_test(app, layout.diff_files, col, row) {
            let _ = <DiffList as UiComponent>::on_event(app, evt);
        }
        return true;
    }
    if rect_contains(layout.diff_preview, col, row) {
        focus::focus_diff_preview(app);
        return true;
    }

    false
}
