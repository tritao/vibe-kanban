use crossterm::event::MouseEvent;

use crate::{
    layout::{compute_main_layout, current_terminal_rect, rect_contains},
    state::AppState,
    ui::components::{
        UiComponent,
        board_pane::{BoardPane, BoardPaneEvent},
        diff_pane::{DiffPane, DiffPaneEvent},
        exec_pane::{ExecPane, ExecPaneEvent},
    },
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
        return <ExecPane as UiComponent>::on_event(
            app,
            ExecPaneEvent::Mouse {
                mouse,
                area: layout.exec,
            },
        );
    }

    if rect_contains(layout.board, col, row) {
        return <BoardPane as UiComponent>::on_event(
            app,
            BoardPaneEvent::Mouse {
                mouse,
                area: layout.board,
            },
        );
    }
    if rect_contains(layout.exec, col, row) {
        return <ExecPane as UiComponent>::on_event(
            app,
            ExecPaneEvent::Mouse {
                mouse,
                area: layout.exec,
            },
        );
    }
    if rect_contains(layout.diff, col, row) {
        return <DiffPane as UiComponent>::on_event(
            app,
            DiffPaneEvent::Mouse {
                mouse,
                area: layout.diff,
            },
        );
    }

    false
}
