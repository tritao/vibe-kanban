use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::{
    diff::diff_rows_with_all,
    layout::{current_terminal_rect, rect_contains},
    state::AppState,
    util::window_for_list,
};

pub(super) fn handle_search_caret_click(app: &mut AppState, mouse: MouseEvent) -> bool {
    let col = mouse.column;
    let row = mouse.row;
    let Some(input) = app.ui.input.as_mut() else {
        return false;
    };
    if app.ui.confirm.is_some() || app.ui.show_help || app.ui.create_task.is_some() {
        return false;
    }
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        return false;
    }

    let area = crate::ui::layout::centered_rect(80, 25, current_terminal_rect());
    let input_y = area.y.saturating_add(1).saturating_add(2);
    let input_x0 = area.x.saturating_add(1).saturating_add(1);
    let input_x1 = area.x.saturating_add(area.width).saturating_sub(2);

    if row != input_y || col < input_x0 || col > input_x1 {
        return false;
    }

    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1);

    let start_col = input.field.scroll_x as usize;
    let left = start_col > 0;
    let click_x = col.saturating_sub(input_x0) as usize;
    let mut target_col = if left {
        if click_x == 0 {
            start_col
        } else {
            start_col.saturating_add(click_x.saturating_sub(1))
        }
    } else {
        start_col.saturating_add(click_x)
    };

    let line_w = crate::text::display_width(&input.field.buffer);
    target_col = target_col.min(line_w);
    input.field.cursor =
        crate::text::edit::byte_index_at_display_col(&input.field.buffer, target_col);
    input.field.goal_col = None;
    input.field.ensure_cursor_visible(content_w, 1);
    true
}

pub(super) fn diff_files_hit_at(app: &AppState, area: Rect, col: u16, row: u16) -> Option<usize> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let rows = diff_rows_with_all(&app.diff.diff_store);
    if rows.is_empty() {
        return None;
    }

    let inner_y0 = area.y.saturating_add(1);
    let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
    if row < inner_y0 || row >= inner_y1 {
        return None;
    }

    let height = area.height.saturating_sub(2) as usize;
    if height == 0 {
        return None;
    }

    let selected = app.diff.selected_diff_index.min(rows.len() - 1);
    let (start, end, _) = window_for_list(rows.len(), selected, height);
    let visible_len = end.saturating_sub(start);

    let inner_row = row.saturating_sub(inner_y0) as usize;
    if inner_row >= visible_len {
        return None;
    }

    Some(start + inner_row)
}

pub(super) fn log_entry_hit_at(
    app: &AppState,
    area: Rect,
    col: u16,
    row: u16,
) -> Option<crate::logs::LogSelection> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let len = app.exec.log_lines.len();
    if len == 0 {
        return None;
    }

    let inner_y0 = area.y.saturating_add(1);
    let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
    if row < inner_y0 || row >= inner_y1 {
        return None;
    }

    let visible = area.height.saturating_sub(2) as usize;
    if visible == 0 {
        return None;
    }

    let visible = visible.min(len);
    let mut offset = if app.exec.log_autoscroll {
        0
    } else {
        app.exec.log_scroll_offset
    };
    offset = offset.min(len.saturating_sub(visible));
    let start = len.saturating_sub(visible + offset);

    let inner_row = row.saturating_sub(inner_y0) as usize;
    if inner_row >= visible {
        return None;
    }

    let line_idx = start.saturating_add(inner_row);
    app.exec.log_line_targets.get(line_idx).and_then(|v| *v)
}
