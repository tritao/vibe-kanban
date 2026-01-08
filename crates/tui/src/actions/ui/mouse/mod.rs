use crossterm::event::MouseEvent;

use crate::{
    layout::{compute_main_layout, current_terminal_rect, rect_contains},
    state::AppState,
    ui::components,
};

pub(super) fn reduce_mouse(app: &mut AppState, mouse: MouseEvent) -> bool {
    let col = mouse.column;
    let row = mouse.row;

    if crate::ui::modals::handle_modal_mouse(app, mouse) {
        return true;
    }
    if crate::ui::modals::modal_blocks_mouse(app) {
        return false;
    }

    let layout = compute_main_layout(current_terminal_rect(), app.ui.focus);

    if app.exec.log_mouse_selecting
        && matches!(
            mouse.kind,
            crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left)
                | crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left)
        )
    {
        return components::handle_exec_mouse(app, mouse, layout.exec);
    }

    if rect_contains(layout.board, col, row) {
        return components::handle_board_mouse(app, mouse, layout.board);
    }
    if rect_contains(layout.exec, col, row) {
        return components::handle_exec_mouse(app, mouse, layout.exec);
    }
    if rect_contains(layout.diff, col, row) {
        return components::handle_diff_mouse(app, mouse, layout.diff);
    }

    false
}
