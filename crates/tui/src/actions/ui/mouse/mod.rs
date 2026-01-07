use crossterm::event::MouseEvent;

use super::focus;
use crate::{
    layout::{compute_main_layout, current_terminal_rect, rect_contains},
    state::AppState,
    ui::components::{
        UiComponent,
        board_pane::{BoardPane, BoardPaneEvent},
        diff_list::{DiffList, DiffListEvent},
        diff_preview::{DiffPreview, DiffPreviewEvent},
        exec_input::ExecInput,
        exec_log::{ExecLog, ExecLogEvent, ExecLogHitKind},
    },
};

mod diff;

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

    const LOG_WHEEL_STEP: usize = 3;
    const DIFF_WHEEL_STEP: usize = 3;

    match mouse.kind {
        crossterm::event::MouseEventKind::ScrollUp => {
            if rect_contains(layout.exec_logs, col, row) {
                focus::focus_execution(app);
                let _ = <ExecLog as UiComponent>::on_event(
                    app,
                    ExecLogEvent::WheelDelta(-(LOG_WHEEL_STEP as i32)),
                );
                return true;
            }
            if rect_contains(layout.diff_preview, col, row) {
                focus::focus_diff_preview(app);
                let _ = <DiffPreview as UiComponent>::on_event(
                    app,
                    DiffPreviewEvent::WheelDelta(-(DIFF_WHEEL_STEP as i32)),
                );
                return true;
            }
            if rect_contains(layout.diff_files, col, row) {
                focus::focus_diff_files(app);
                let _ = <DiffList as UiComponent>::on_event(app, DiffListEvent::WheelDelta(-1));
                return true;
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
            if rect_contains(layout.exec_logs, col, row) {
                focus::focus_execution(app);
                let _ = <ExecLog as UiComponent>::on_event(
                    app,
                    ExecLogEvent::WheelDelta(LOG_WHEEL_STEP as i32),
                );
                return true;
            }
            if rect_contains(layout.diff_preview, col, row) {
                focus::focus_diff_preview(app);
                let _ = <DiffPreview as UiComponent>::on_event(
                    app,
                    DiffPreviewEvent::WheelDelta(DIFF_WHEEL_STEP as i32),
                );
                return true;
            }
            if rect_contains(layout.diff_files, col, row) {
                focus::focus_diff_files(app);
                let _ = <DiffList as UiComponent>::on_event(app, DiffListEvent::WheelDelta(1));
                return true;
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
            if rect_contains(layout.exec_input, col, row) {
                focus::focus_execution(app);
                if let Some(evt) =
                    <ExecInput as UiComponent>::hit_test(app, layout.exec_input, col, row)
                {
                    return <ExecInput as UiComponent>::on_event(app, evt);
                }
                return true;
            }
            if rect_contains(layout.exec_logs, col, row) {
                focus::focus_execution(app);
                if let Some(evt) =
                    <ExecLog as UiComponent>::hit_test(app, layout.exec_logs, col, row)
                {
                    return <ExecLog as UiComponent>::on_event(app, evt);
                }
                return true;
            }
            if rect_contains(layout.diff, col, row) {
                return diff::handle_diff_left_click(app, mouse, &layout);
            }
        }
        crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
            if rect_contains(layout.exec_logs, col, row) {
                focus::focus_execution(app);
                let Some(ExecLogEvent::Hit(_, hit)) =
                    <ExecLog as UiComponent>::hit_test(app, layout.exec_logs, col, row)
                else {
                    return false;
                };
                let Some(line_idx) = hit.line_idx else {
                    return false;
                };
                return <ExecLog as UiComponent>::on_event(app, ExecLogEvent::DragTo(line_idx));
            }
        }
        crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
            if app.exec.log_mouse_selecting {
                return <ExecLog as UiComponent>::on_event(app, ExecLogEvent::DragEnd);
            }
        }
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
            if rect_contains(layout.exec_logs, col, row) {
                focus::focus_execution(app);
                let Some(ExecLogEvent::Hit(_, hit)) =
                    <ExecLog as UiComponent>::hit_test(app, layout.exec_logs, col, row)
                else {
                    return false;
                };
                return <ExecLog as UiComponent>::on_event(
                    app,
                    ExecLogEvent::Hit(ExecLogHitKind::Right, hit),
                );
            }
        }
        _ => {}
    }

    false
}
