use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::UiComponent;
use crate::{
    layout::rect_contains,
    state::{AppState, FocusPane},
    text::{display_width, slice_by_display_cols},
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExecInputEvent {
    ClickTo {
        cursor: usize,
        content_w: usize,
        inner_h: usize,
    },
}

pub(crate) struct ExecInput;

impl ExecInput {
    fn border_style(app: &AppState) -> Style {
        if app.ui.focus == FocusPane::Execution {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        }
    }

    fn compute_click_cursor(app: &AppState, area: Rect, col: u16, row: u16) -> ExecInputEvent {
        let inner_w = area.width.saturating_sub(2) as usize;
        let inner_h = area.height.saturating_sub(2) as usize;
        let prefix_w = display_width("  ");
        let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);

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
}

impl UiComponent for ExecInput {
    type Event = ExecInputEvent;

    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        let inner_w = area.width.saturating_sub(2) as usize;
        let inner_h = area.height.saturating_sub(2) as usize;

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
                    .border_style(Self::border_style(app)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(w, area);
    }

    fn hit_test(app: &AppState, area: Rect, col: u16, row: u16) -> Option<Self::Event> {
        if !rect_contains(area, col, row) {
            return None;
        }
        Some(Self::compute_click_cursor(app, area, col, row))
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        if app.ui.focus != FocusPane::Execution {
            return false;
        }

        match event {
            ExecInputEvent::ClickTo {
                cursor,
                content_w,
                inner_h,
            } => {
                app.ui.composer_active = true;
                app.ui.composer_suggest_index = 0;
                app.ui.composer.cursor = cursor.min(app.ui.composer.buffer.len());
                app.ui.composer.goal_col = None;
                app.ui.composer.ensure_cursor_visible(content_w, inner_h);
                true
            }
        }
    }
}
