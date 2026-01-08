use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    state::{AppState, FocusPane},
    text::{display_width, slice_by_display_cols},
};

fn border_style(app: &AppState) -> Style {
    if app.ui.focus == FocusPane::Execution {
        crate::ui::palette::border_active()
    } else {
        Style::default()
    }
}

pub(super) fn render_exec_input(f: &mut Frame, app: &AppState, area: Rect) {
    let (inner_w, inner_h) = crate::layout::inner_wh(area);

    let lines: Vec<Line<'static>> = if app.ui.composer_active {
        use crate::text::edit::line_ranges;

        // Keep 1 cell free so the terminal cursor can sit "after" the last character.
        let inner_h = inner_h.max(1);
        let prefix = "  ";
        let prefix_w = display_width(prefix);
        let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);

        let ranges = line_ranges(&app.ui.composer.buffer);
        let total_lines = ranges.len().max(1);

        let (cur_line, cur_col) = app.ui.composer.cursor_line_col();
        let cur_line = cur_line.min(total_lines.saturating_sub(1));

        let start_line = (app.ui.composer.scroll_y as usize).min(total_lines.saturating_sub(1));
        let end_line = (start_line + inner_h).min(total_lines);

        let mut out: Vec<Line<'static>> = Vec::with_capacity(inner_h);
        for (idx, (start, end)) in ranges.iter().enumerate().take(end_line).skip(start_line) {
            let line_str = app.ui.composer.buffer.get(*start..*end).unwrap_or("");

            let prefix = if idx == start_line {
                if start_line > 0 { "… " } else { "> " }
            } else {
                "  "
            };

            let line_w = display_width(line_str);
            let start_col = app.ui.composer.scroll_x as usize;
            let left = start_col > 0;

            let mut right = false;
            let mut take = content_w.saturating_sub(left as usize);
            if idx != cur_line && start_col.saturating_add(take) < line_w {
                right = true;
                take = content_w.saturating_sub(left as usize).saturating_sub(1);
            }

            let mut visible = String::new();
            if left {
                visible.push('…');
            }
            visible.push_str(&slice_by_display_cols(line_str, start_col, take));
            if right {
                visible.push('…');
            }

            if idx == cur_line {
                let cursor_in_chunk = cur_col.saturating_sub(start_col).min(take);
                let cursor_x_in_visible = (left as usize).saturating_add(cursor_in_chunk);
                let cursor_x = area
                    .x
                    .saturating_add(1)
                    .saturating_add(prefix_w as u16)
                    .saturating_add(cursor_x_in_visible as u16)
                    .min(area.x.saturating_add(area.width).saturating_sub(2));
                let cursor_y = area
                    .y
                    .saturating_add(1)
                    .saturating_add((idx - start_line) as u16);
                f.set_cursor_position((cursor_x, cursor_y));
            }

            out.push(Line::from(format!("{prefix}{visible}")));
        }

        if out.is_empty() {
            out.push(Line::from("> "));
            f.set_cursor_position((area.x.saturating_add(3), area.y.saturating_add(1)));
        }

        out
    } else {
        vec![Line::from("Press i to type a follow-up or /command…")]
    };

    let w = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input")
                .border_style(border_style(app)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(w, area);
}
