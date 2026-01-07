use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::{layout::current_terminal_rect, state::AppState};

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
