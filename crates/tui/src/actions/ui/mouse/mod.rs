use crossterm::event::MouseEvent;

use super::focus;
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

    match mouse.kind {
        crossterm::event::MouseEventKind::ScrollUp => {
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
            if let Some(hit) =
                crate::ui::components::board_pane::board_hit_at(app, layout.board, col, row)
            {
                focus::focus_board(app);
                let _ = <BoardPane as UiComponent>::on_event(
                    app,
                    BoardPaneEvent::Wheel {
                        status: hit.status,
                        delta: -1,
                    },
                );
                return true;
            }
        }
        crossterm::event::MouseEventKind::ScrollDown => {
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
            if let Some(hit) =
                crate::ui::components::board_pane::board_hit_at(app, layout.board, col, row)
            {
                focus::focus_board(app);
                let _ = <BoardPane as UiComponent>::on_event(
                    app,
                    BoardPaneEvent::Wheel {
                        status: hit.status,
                        delta: 1,
                    },
                );
                return true;
            }
        }
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            if rect_contains(layout.board, col, row) {
                focus::focus_board(app);
                if let Some(evt) = <BoardPane as UiComponent>::hit_test(app, layout.board, col, row)
                {
                    return <BoardPane as UiComponent>::on_event(app, evt);
                }
                return true;
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
        }
        crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
            if rect_contains(layout.exec, col, row) {
                return <ExecPane as UiComponent>::on_event(
                    app,
                    ExecPaneEvent::Mouse {
                        mouse,
                        area: layout.exec,
                    },
                );
            }
        }
        crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
            if app.exec.log_mouse_selecting {
                return <ExecPane as UiComponent>::on_event(
                    app,
                    ExecPaneEvent::Mouse {
                        mouse,
                        area: layout.exec,
                    },
                );
            }
        }
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
            if rect_contains(layout.exec, col, row) {
                return <ExecPane as UiComponent>::on_event(
                    app,
                    ExecPaneEvent::Mouse {
                        mouse,
                        area: layout.exec,
                    },
                );
            }
        }
        _ => {}
    }

    false
}
