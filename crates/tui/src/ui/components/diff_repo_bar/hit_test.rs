use ratatui::layout::Rect;

use super::{DiffRepoAction, layout};
use crate::{layout::rect_contains, state::AppState, ui::button_row::hit_test_button_row};

pub(super) fn diff_repo_bar_action_at(
    app: &AppState,
    area: Rect,
    col: u16,
    row: u16,
) -> Option<DiffRepoAction> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let inner_x0 = area.x.saturating_add(1);
    let inner_x1 = area.x.saturating_add(area.width).saturating_sub(1);
    let inner_y0 = area.y.saturating_add(1);
    let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
    if col < inner_x0 || col >= inner_x1 || row < inner_y0 || row >= inner_y1 {
        return None;
    }

    let w = area.width.saturating_sub(2) as usize;
    let inner_col = col.saturating_sub(inner_x0) as usize;
    let layout::DiffRepoBarLayout {
        can_show_right,
        buttons,
        button_row_start_col,
        ..
    } = layout::compute_repo_bar_layout(app, w, std::time::Instant::now());

    if !can_show_right {
        return None;
    }

    hit_test_button_row(&buttons, inner_col, button_row_start_col)
}
