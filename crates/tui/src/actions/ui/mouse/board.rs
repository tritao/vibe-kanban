use crossterm::event::MouseEvent;

use super::super::{focus, sel};
use crate::{layout::rect_contains, state::AppState, ui::board_hit_at};

pub(super) fn handle_board_left_click(
    app: &mut AppState,
    mouse: MouseEvent,
    area: ratatui::layout::Rect,
) -> bool {
    let col = mouse.column;
    let row = mouse.row;
    if !rect_contains(area, col, row) {
        return false;
    }

    focus::focus_board(app);
    if let Some(hit) = board_hit_at(app, area, col, row) {
        sel::apply_board_hit(app, hit);
        return true;
    }
    true
}
