use crossterm::event::MouseEvent;

use super::{focus, modals, scroll, sel};
use crate::{
    layout::{compute_main_layout, current_terminal_rect, rect_contains},
    state::AppState,
    ui::board_hit_at,
};

mod board;
mod diff;
mod exec;
mod hit_test;

pub(super) fn reduce_mouse(app: &mut AppState, mouse: MouseEvent) -> bool {
    let col = mouse.column;
    let row = mouse.row;

    // Search modal caret placement.
    if hit_test::handle_search_caret_click(app, mouse) {
        return true;
    }
    if app.ui.input.is_some() {
        return false;
    }

    if modals::modal_blocks_mouse(app) {
        return false;
    }

    let layout = compute_main_layout(current_terminal_rect(), app.ui.focus);

    const LOG_WHEEL_STEP: usize = 3;
    const DIFF_WHEEL_STEP: usize = 3;

    match mouse.kind {
        crossterm::event::MouseEventKind::ScrollUp => {
            if rect_contains(layout.exec_logs, col, row) {
                focus::focus_execution(app);
                scroll::scroll_exec_older(app, LOG_WHEEL_STEP);
                return true;
            }
            if rect_contains(layout.diff_preview, col, row) {
                focus::focus_diff_preview(app);
                scroll::scroll_diff_up(app, DIFF_WHEEL_STEP);
                return true;
            }
            if rect_contains(layout.diff_files, col, row) {
                focus::focus_diff_files(app);
                sel::select_adjacent_diff_file(app, -1);
                return true;
            }
            if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                focus::focus_board(app);
                sel::focus_board_section(app, hit.status);
                sel::select_adjacent_task(app, -1);
                return true;
            }
        }
        crossterm::event::MouseEventKind::ScrollDown => {
            if rect_contains(layout.exec_logs, col, row) {
                focus::focus_execution(app);
                scroll::scroll_exec_newer(app, LOG_WHEEL_STEP);
                return true;
            }
            if rect_contains(layout.diff_preview, col, row) {
                focus::focus_diff_preview(app);
                scroll::scroll_diff_down(app, DIFF_WHEEL_STEP);
                return true;
            }
            if rect_contains(layout.diff_files, col, row) {
                focus::focus_diff_files(app);
                sel::select_adjacent_diff_file(app, 1);
                return true;
            }
            if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                focus::focus_board(app);
                sel::focus_board_section(app, hit.status);
                sel::select_adjacent_task(app, 1);
                return true;
            }
        }
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            if rect_contains(layout.board, col, row) {
                return board::handle_board_left_click(app, mouse, layout.board);
            }
            if rect_contains(layout.exec, col, row) {
                return exec::handle_exec_left_click(app, mouse, &layout);
            }
            if rect_contains(layout.diff, col, row) {
                return diff::handle_diff_left_click(app, mouse, &layout);
            }
        }
        crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
            if rect_contains(layout.exec_logs, col, row) {
                return exec::handle_exec_right_click(app, mouse, &layout);
            }
        }
        _ => {}
    }

    false
}
