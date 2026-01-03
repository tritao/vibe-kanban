use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::diff::diff_rows_with_all;
use crate::layout::{compute_main_layout, current_terminal_rect, rect_contains};
use crate::state::{AppState, DiffFocus, FocusPane};
use crate::ui::board_hit_at;
use crate::util::window_for_list;

use super::sel;

pub(super) fn reduce_mouse(app: &mut AppState, mouse: MouseEvent) -> bool {
    let col = mouse.column;
    let row = mouse.row;

    // Search modal caret placement.
    if let Some(input) = app.ui.input.as_mut() {
        if app.ui.confirm.is_some() || app.ui.show_help || app.ui.create_task.is_some() {
            return false;
        }
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            let area = crate::ui::layout::centered_rect(80, 25, current_terminal_rect());
            let input_y = area.y.saturating_add(1).saturating_add(2);
            let input_x0 = area.x.saturating_add(1).saturating_add(1);
            let input_x1 = area.x.saturating_add(area.width).saturating_sub(2);

            if row == input_y && col >= input_x0 && col <= input_x1 {
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
                return true;
            }
        }
        return false;
    }

    if app.ui.confirm.is_some() || app.ui.show_help || app.ui.create_task.is_some() {
        return false;
    }

    let layout = compute_main_layout(current_terminal_rect());

    const LOG_WHEEL_STEP: usize = 3;
    const DIFF_WHEEL_STEP: usize = 3;

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if rect_contains(layout.exec_logs, col, row) {
                app.ui.focus = FocusPane::Execution;
                app.exec.log_autoscroll = false;
                app.exec.log_scroll_offset =
                    app.exec.log_scroll_offset.saturating_add(LOG_WHEEL_STEP);
                return true;
            }
            if rect_contains(layout.diff_preview, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Preview;
                app.diff.diff_scroll_offset =
                    app.diff.diff_scroll_offset.saturating_sub(DIFF_WHEEL_STEP);
                return true;
            }
            if rect_contains(layout.diff_files, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Files;
                sel::select_adjacent_diff_file(app, -1);
                return true;
            }
            if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                app.ui.focus = FocusPane::Board;
                sel::focus_board_section(app, hit.status);
                sel::select_adjacent_task(app, -1);
                return true;
            }
        }
        MouseEventKind::ScrollDown => {
            if rect_contains(layout.exec_logs, col, row) {
                app.ui.focus = FocusPane::Execution;
                app.exec.log_scroll_offset =
                    app.exec.log_scroll_offset.saturating_sub(LOG_WHEEL_STEP);
                if app.exec.log_scroll_offset == 0 {
                    app.exec.log_autoscroll = true;
                }
                return true;
            }
            if rect_contains(layout.diff_preview, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Preview;
                app.diff.diff_scroll_offset =
                    app.diff.diff_scroll_offset.saturating_add(DIFF_WHEEL_STEP);
                return true;
            }
            if rect_contains(layout.diff_files, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Files;
                sel::select_adjacent_diff_file(app, 1);
                return true;
            }
            if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                app.ui.focus = FocusPane::Board;
                sel::focus_board_section(app, hit.status);
                sel::select_adjacent_task(app, 1);
                return true;
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if rect_contains(layout.board, col, row) {
                app.ui.focus = FocusPane::Board;
                if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                    sel::apply_board_hit(app, hit);
                }
                return true;
            }

            if rect_contains(layout.exec, col, row) {
                app.ui.focus = FocusPane::Execution;
                if rect_contains(layout.exec_input, col, row) {
                    app.ui.composer_active = true;
                    let area = layout.exec_input;
                    let inner_w = area.width.saturating_sub(2) as usize;
                    let inner_h = area.height.saturating_sub(2) as usize;
                    let prefix_w = crate::text::display_width("  ");
                    let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);

                    // Click-to-place caret (same layout assumptions as render_composer).
                    let inner_x0 = area.x.saturating_add(1);
                    let inner_y0 = area.y.saturating_add(1);
                    if row >= inner_y0 {
                        let rel_y = row.saturating_sub(inner_y0) as usize;
                        let line_ranges =
                            crate::text::edit::line_ranges(&app.ui.composer.buffer);
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
                            let line_w = crate::text::display_width(line_str);
                            target_col = target_col.min(line_w);

                            let within =
                                crate::text::edit::byte_index_at_display_col(line_str, target_col);
                            app.ui.composer.cursor =
                                (ls + within).min(app.ui.composer.buffer.len());
                            app.ui.composer.goal_col = None;
                        } else {
                            app.ui.composer.set_end();
                        }
                    } else {
                        app.ui.composer.set_end();
                    }

                    app.ui.composer.ensure_cursor_visible(content_w, inner_h.max(1));
                    return true;
                }
                if rect_contains(layout.exec_logs, col, row) {
                    sel::select_log_entry(app, log_entry_hit_at(app, layout.exec_logs, col, row));
                    return true;
                }
                return false;
            }

            if rect_contains(layout.diff, col, row) {
                app.ui.focus = FocusPane::Diff;
                if rect_contains(layout.diff_repo_bar, col, row) {
                    if let Some(action) =
                        crate::ui::diff_repo_bar_action_at(app, layout.diff_repo_bar, col, row)
                    {
                        crate::ui::trigger_diff_repo_action(app, action);
                    }
                    return true;
                }
                if rect_contains(layout.diff_files, col, row) {
                    app.ui.diff_focus = DiffFocus::Files;
                    if let Some(idx) = diff_files_hit_at(app, layout.diff_files, col, row) {
                        sel::select_diff_file(app, idx);
                    }
                    return true;
                }
                if rect_contains(layout.diff_preview, col, row) {
                    app.ui.diff_focus = DiffFocus::Preview;
                    return true;
                }
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            if rect_contains(layout.exec_logs, col, row) {
                app.ui.focus = FocusPane::Execution;
                sel::select_log_entry(app, log_entry_hit_at(app, layout.exec_logs, col, row));
                crate::logs::toggle_selected_log_entry(app);
                return true;
            }
        }
        _ => {}
    }

    false
}

fn diff_files_hit_at(app: &AppState, area: Rect, col: u16, row: u16) -> Option<usize> {
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

fn log_entry_hit_at(
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
