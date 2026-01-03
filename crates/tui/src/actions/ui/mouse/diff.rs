use crossterm::event::MouseEvent;

use crate::layout::rect_contains;
use crate::state::AppState;

use super::hit_test::diff_files_hit_at;
use super::super::focus;
use super::super::sel;

pub(super) fn handle_diff_left_click(
    app: &mut AppState,
    mouse: MouseEvent,
    layout: &crate::layout::MainLayoutRects,
) -> bool {
    let col = mouse.column;
    let row = mouse.row;

    if rect_contains(layout.diff_repo_bar, col, row) {
        focus::focus_diff(app);
        if let Some(action) = crate::ui::diff_repo_bar_action_at(app, layout.diff_repo_bar, col, row)
        {
            crate::ui::trigger_diff_repo_action(app, action);
        }
        return true;
    }
    if rect_contains(layout.diff_files, col, row) {
        focus::focus_diff_files(app);
        if let Some(idx) = diff_files_hit_at(app, layout.diff_files, col, row) {
            sel::select_diff_file(app, idx);
        }
        return true;
    }
    if rect_contains(layout.diff_preview, col, row) {
        focus::focus_diff_preview(app);
        return true;
    }

    false
}
