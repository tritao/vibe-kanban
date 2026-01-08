use ratatui::layout::Rect;

use super::ExecInputEvent;
use crate::{state::AppState, text::display_width};

pub(super) fn compute_click_cursor(
    app: &AppState,
    area: Rect,
    col: u16,
    row: u16,
) -> ExecInputEvent {
    let (_inner_w, _) = crate::ui::geometry::inner_size(area);
    let inner_h = area.height.saturating_sub(2) as usize;
    let prefix_w = display_width("  ");
    let content_w = crate::ui::geometry::inner_content_width(area, prefix_w, 1);

    let inner_x0 = area.x.saturating_add(1);
    let inner_y0 = area.y.saturating_add(1);

    let cursor = if row >= inner_y0 {
        let rel_y = row.saturating_sub(inner_y0) as usize;
        let line_ranges = crate::text::edit::line_ranges(&app.ui.composer.buffer);
        if !line_ranges.is_empty() {
            let start_line = app.ui.composer.scroll_y as usize;
            let target_line = start_line
                .saturating_add(rel_y)
                .min(line_ranges.len().saturating_sub(1));
            let (ls, le) = line_ranges[target_line];
            let line_str = app.ui.composer.buffer.get(ls..le).unwrap_or("");

            let content_x0 = inner_x0.saturating_add(prefix_w as u16);
            let mut rel_x = col.saturating_sub(content_x0) as usize;

            let start_col = app.ui.composer.scroll_x as usize;
            let left = start_col > 0;
            if left && rel_x > 0 {
                rel_x = rel_x.saturating_sub(1);
            } else if left && rel_x == 0 {
                rel_x = 0;
            }
            let mut target_col = start_col.saturating_add(rel_x);
            let line_w = display_width(line_str);
            target_col = target_col.min(line_w);

            let within = crate::text::edit::byte_index_at_display_col(line_str, target_col);
            (ls + within).min(app.ui.composer.buffer.len())
        } else {
            app.ui.composer.buffer.len()
        }
    } else {
        app.ui.composer.buffer.len()
    };

    ExecInputEvent::ClickTo {
        cursor,
        content_w,
        inner_h: inner_h.max(1),
    }
}
