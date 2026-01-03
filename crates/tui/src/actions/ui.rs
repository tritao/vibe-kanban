use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::commands::{copy_to_clipboard_osc52, submit_composer, set_toast};
use crate::events::UiEvent;
use crate::layout::{compute_main_layout, current_terminal_rect, rect_contains};
use crate::prefs::save_prefs;
use crate::diff::diff_rows_with_all;
use crate::selection::{
    ensure_selected_task_in_active_column, ensure_selection_visible, move_active_status,
    request_move_selected_task, select_adjacent_attempt, select_adjacent_diff_file,
    select_adjacent_task, set_selected_task,
};
use crate::state::{
    AppState, ConfirmAction, ConfirmState, DiffFocus, FocusPane, InputMode, InputState, LogMode,
    LogRenderMode, LogViewMode, TaskStatus,
};
use crate::ui::{board_hit_at, diff_repo_bar_action_at, open_create_task_modal, sync_selected_repo_from_diff_selection, trigger_diff_repo_action, DiffRepoAction};
use crate::util::window_for_list;

pub(super) enum Effect {
    CopyOsc52(String),
    Toast {
        message: String,
        color: Color,
        expires_at: Option<Instant>,
    },
}

#[derive(Debug, Clone, Copy)]
enum CopyTarget {
    Execution,
    DiffFiles,
    DiffPreview,
}

pub(super) fn reduce_ui(app: &mut AppState, evt: UiEvent) -> anyhow::Result<(bool, bool, Vec<Effect>)> {
    match evt {
        UiEvent::Tick => Ok((false, false, vec![])),
        UiEvent::Crossterm(Event::Key(key)) => {
            if key.kind != KeyEventKind::Press {
                return Ok((false, false, vec![]));
            }
            Ok(reduce_key(app, key))
        }
        UiEvent::Crossterm(Event::Mouse(mouse)) => Ok((false, reduce_mouse(app, mouse), vec![])),
        _ => Ok((false, false, vec![])),
    }
}

pub(super) fn run_effects(app: &mut AppState, effects: Vec<Effect>) -> bool {
    let mut dirty = false;
    for eff in effects {
        match eff {
            Effect::CopyOsc52(text) => {
                if let Err(e) = copy_to_clipboard_osc52(&text) {
                    set_toast(
                        app,
                        format!("Copy failed: {e}"),
                        Color::Red,
                        Some(Instant::now() + std::time::Duration::from_secs(2)),
                    );
                    dirty = true;
                }
            }
            Effect::Toast {
                message,
                color,
                expires_at,
            } => {
                set_toast(app, message, color, expires_at);
                dirty = true;
            }
        }
    }
    dirty
}

fn reduce_key(app: &mut AppState, key: crossterm::event::KeyEvent) -> (bool, bool, Vec<Effect>) {
    use crossterm::event::{KeyCode, KeyModifiers};

    // Confirm modal has highest priority.
    if let Some(confirm) = app.ui.confirm.as_ref() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                let action = confirm.action;
                app.ui.confirm = None;
                crate::handle_confirm_action(app, action);
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.ui.confirm = None;
            }
            _ => {}
        }
        return (false, true, vec![]);
    }

    // Search modal.
    if app.ui.input.is_some() {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                if let Some(input) = app.ui.input.take() {
                    app.board.task_filter = input.original;
                }
                ensure_selection_visible(app);
                return (false, true, vec![]);
            }
            (KeyCode::Enter, _) => {
                app.ui.input = None;
                ensure_selection_visible(app);
                return (false, true, vec![]);
            }
            _ => {}
        }
        if let Some((dirty, effects)) = reduce_search_key(app, key) {
            return (false, dirty, effects);
        }
        return (false, false, vec![]);
    }

    // Help modal.
    if app.ui.show_help {
        match key.code {
            KeyCode::Char('?') | KeyCode::Esc => {
                app.ui.show_help = false;
                return (false, true, vec![]);
            }
            _ => return (false, false, vec![]),
        }
    }

    // Create-task modal.
    if app.ui.create_task.is_some() {
        reduce_create_task_key(app, key);
        return (false, true, vec![]);
    }

    // Composer editing.
    if app.ui.composer_active {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                app.ui.composer_active = false;
                app.ui.composer.clear();
                app.ui.composer_suggest_index = 0;
                return (false, true, vec![]);
            }
            (KeyCode::Enter, _) => {
                submit_composer(app);
                return (false, true, vec![]);
            }
            _ => {}
        }
        if let Some((dirty, effects)) = reduce_composer_key(app, key) {
            return (false, dirty, effects);
        }
        return (false, false, vec![]);
    }

    // Global actions.
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => return (true, false, vec![]),
        (KeyCode::Char('?'), _) => {
            app.ui.show_help = true;
            return (false, true, vec![]);
        }
        (KeyCode::Tab, KeyModifiers::NONE) => {
            app.ui.focus = match app.ui.focus {
                FocusPane::Board => FocusPane::Execution,
                FocusPane::Execution => FocusPane::Diff,
                FocusPane::Diff => FocusPane::Board,
            };
            return (false, true, vec![]);
        }
        (KeyCode::Char('/'), _) => {
            let mut field = crate::state::TextFieldState::default();
            field.buffer = app.board.task_filter.clone();
            field.set_end();
            let term = current_terminal_rect();
            let area = crate::ui::layout::centered_rect(80, 25, term);
            let inner_w = area.width.saturating_sub(2) as usize;
            let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1);
            field.ensure_cursor_visible(content_w, 1);
            app.ui.input = Some(InputState {
                mode: InputMode::SearchTasks,
                field,
                original: app.board.task_filter.clone(),
            });
            return (false, true, vec![]);
        }
        (KeyCode::Char('r'), _) => {
            let next = *app.reconnect_tx.borrow() + 1;
            let _ = app.reconnect_tx.send(next);
            return (false, true, vec![]);
        }
        (KeyCode::Char('o'), _) => {
            app.exec.log_mode = match app.exec.log_mode {
                LogMode::Normalized => LogMode::Raw,
                LogMode::Raw => LogMode::Normalized,
            };
            crate::logs::reset_logs(app, None);
            let _ = app.log_mode_tx.send(app.exec.log_mode);
            app.prefs.log_mode = app.exec.log_mode;
            save_prefs(&app.prefs);
            return (false, true, vec![]);
        }
        (KeyCode::Char('m'), _) if app.ui.focus == FocusPane::Execution => {
            app.exec.log_render_mode = match app.exec.log_render_mode {
                LogRenderMode::Plain => LogRenderMode::Markdown,
                LogRenderMode::Markdown => LogRenderMode::Plain,
            };
            app.prefs.log_render_mode = app.exec.log_render_mode;
            save_prefs(&app.prefs);
            crate::logs::mark_all_log_buffers_dirty(app, 0);
            return (false, true, vec![]);
        }
        (KeyCode::Char('v'), _) if app.ui.focus == FocusPane::Execution => {
            app.exec.log_view_mode = match app.exec.log_view_mode {
                LogViewMode::Timeline => LogViewMode::Single,
                LogViewMode::Single => LogViewMode::Timeline,
            };
            app.prefs.log_view_mode = app.exec.log_view_mode;
            save_prefs(&app.prefs);
            app.exec.log_view_dirty = true;
            return (false, true, vec![]);
        }
        (KeyCode::Char('y'), _) => {
            let target = match app.ui.focus {
                FocusPane::Execution => CopyTarget::Execution,
                FocusPane::Diff => match app.ui.diff_focus {
                    DiffFocus::Files => CopyTarget::DiffFiles,
                    DiffFocus::Preview => CopyTarget::DiffPreview,
                },
                _ => return (false, false, vec![]),
            };
            return (false, false, reduce_copy(app, target));
        }
        (KeyCode::Char('e'), _) | (KeyCode::Enter, _) if app.ui.focus == FocusPane::Execution => {
            crate::logs::toggle_selected_log_entry(app);
            return (false, true, vec![]);
        }
        (KeyCode::Char('d'), _) => {
            app.diff.diff_stats_only = !app.diff.diff_stats_only;
            let _ = app.diff_stats_tx.send(app.diff.diff_stats_only);
            app.diff.diff_scroll_offset = 0;
            return (false, true, vec![]);
        }
        (KeyCode::Char('t'), _) if app.ui.focus == FocusPane::Diff => {
            app.diff.diff_theme = app.diff.diff_theme.cycle_next();
            app.prefs.diff_theme = app.diff.diff_theme;
            save_prefs(&app.prefs);
            app.diff.diff_preview_cache_key = None;
            return (false, true, vec![]);
        }
        (KeyCode::Char('i'), _) if app.ui.focus == FocusPane::Execution => {
            app.ui.composer_active = true;
            app.ui.composer_suggest_index = 0;
            app.ui.composer.set_end();
            let layout = compute_main_layout(current_terminal_rect());
            let area = layout.exec_input;
            let inner_w = area.width.saturating_sub(2) as usize;
            let inner_h = area.height.saturating_sub(2) as usize;
            let prefix_w = crate::text::display_width("  ");
            let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);
            app.ui.composer.ensure_cursor_visible(content_w, inner_h.max(1));
            return (false, true, vec![]);
        }
        (KeyCode::Char('x'), _) => {
            if let Some(exec_id) = app.exec.selected_exec_id {
                app.ui.confirm = Some(ConfirmState {
                    title: "Stop execution?".to_string(),
                    body: format!("Stop execution process {exec_id}? (y/n)"),
                    action: ConfirmAction::StopExec { exec_id },
                });
                return (false, true, vec![]);
            }
        }
        (KeyCode::Char('['), _) => {
            select_adjacent_attempt(app, -1);
            return (false, true, vec![]);
        }
        (KeyCode::Char(']'), _) => {
            select_adjacent_attempt(app, 1);
            return (false, true, vec![]);
        }
        (KeyCode::Char('h'), _) if app.ui.focus == FocusPane::Diff => {
            app.ui.diff_focus = DiffFocus::Files;
            return (false, true, vec![]);
        }
        (KeyCode::Char('l'), _) if app.ui.focus == FocusPane::Diff => {
            app.ui.diff_focus = DiffFocus::Preview;
            return (false, true, vec![]);
        }
        (KeyCode::Char('w'), _) if app.ui.focus == FocusPane::Diff => {
            app.diff.diff_wrap = !app.diff.diff_wrap;
            app.prefs.diff_wrap = app.diff.diff_wrap;
            save_prefs(&app.prefs);
            app.diff.diff_preview_cache_key = None;
            return (false, true, vec![]);
        }
        (KeyCode::Char('S'), _) if app.ui.focus == FocusPane::Diff => {
            trigger_diff_repo_action(app, DiffRepoAction::RefreshStatus);
            return (false, true, vec![]);
        }
        (KeyCode::Char('M'), _) if app.ui.focus == FocusPane::Diff => {
            trigger_diff_repo_action(app, DiffRepoAction::Merge);
            return (false, true, vec![]);
        }
        (KeyCode::Char('R'), _) if app.ui.focus == FocusPane::Diff => {
            trigger_diff_repo_action(app, DiffRepoAction::Rebase);
            return (false, true, vec![]);
        }
        (KeyCode::Char('P'), _) if app.ui.focus == FocusPane::Diff => {
            trigger_diff_repo_action(app, DiffRepoAction::CreatePr);
            return (false, true, vec![]);
        }
        (KeyCode::Char('C'), _) if app.ui.focus == FocusPane::Diff => {
            trigger_diff_repo_action(app, DiffRepoAction::ResolveConflicts);
            return (false, true, vec![]);
        }
        (KeyCode::Char('O'), _) if app.ui.focus == FocusPane::Diff => {
            trigger_diff_repo_action(app, DiffRepoAction::OpenConflict);
            return (false, true, vec![]);
        }
        (KeyCode::Char('A'), _) if app.ui.focus == FocusPane::Diff => {
            trigger_diff_repo_action(app, DiffRepoAction::AbortConflicts);
            return (false, true, vec![]);
        }
        (KeyCode::Char('U'), _) if app.ui.focus == FocusPane::Diff => {
            trigger_diff_repo_action(app, DiffRepoAction::OpenPr);
            return (false, true, vec![]);
        }
        (KeyCode::Enter, _) if app.ui.focus == FocusPane::Diff => {
            trigger_diff_repo_action(app, DiffRepoAction::OpenPr);
            return (false, true, vec![]);
        }
        (KeyCode::Char('c'), _) if app.ui.focus == FocusPane::Board => {
            app.board.show_cancelled = !app.board.show_cancelled;
            if !app.board.show_cancelled && app.board.tasks_active_column == TaskStatus::Cancelled {
                app.board.tasks_active_column = TaskStatus::Done;
            }
            app.prefs.show_cancelled = app.board.show_cancelled;
            save_prefs(&app.prefs);
            return (false, true, vec![]);
        }
        (KeyCode::Char('n'), _) if app.ui.focus == FocusPane::Board => {
            open_create_task_modal(app);
            return (false, true, vec![]);
        }
        (KeyCode::Char('K'), _) if app.ui.focus == FocusPane::Board => {
            move_active_status(app, -1);
            return (false, true, vec![]);
        }
        (KeyCode::Char('J'), _) if app.ui.focus == FocusPane::Board => {
            move_active_status(app, 1);
            return (false, true, vec![]);
        }
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) if app.ui.focus == FocusPane::Board => {
            select_adjacent_task(app, -1);
            return (false, true, vec![]);
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) if app.ui.focus == FocusPane::Board => {
            select_adjacent_task(app, 1);
            return (false, true, vec![]);
        }
        (KeyCode::Left, _) if app.ui.focus == FocusPane::Board => {
            request_move_selected_task(app, -1);
            return (false, true, vec![]);
        }
        (KeyCode::Right, _) if app.ui.focus == FocusPane::Board => {
            request_move_selected_task(app, 1);
            return (false, true, vec![]);
        }
        (KeyCode::Up, _) | (KeyCode::Char('k'), _)
            if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files =>
        {
            select_adjacent_diff_file(app, -1);
            return (false, true, vec![]);
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _)
            if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files =>
        {
            select_adjacent_diff_file(app, 1);
            return (false, true, vec![]);
        }
        (KeyCode::PageUp, _) if app.ui.focus == FocusPane::Execution => {
            app.exec.log_autoscroll = false;
            app.exec.log_scroll_offset = app.exec.log_scroll_offset.saturating_add(40);
            return (false, true, vec![]);
        }
        (KeyCode::PageDown, _) if app.ui.focus == FocusPane::Execution => {
            app.exec.log_scroll_offset = app.exec.log_scroll_offset.saturating_sub(40);
            if app.exec.log_scroll_offset == 0 {
                app.exec.log_autoscroll = true;
            }
            return (false, true, vec![]);
        }
        (KeyCode::End, _) if app.ui.focus == FocusPane::Execution => {
            app.exec.log_autoscroll = true;
            app.exec.log_scroll_offset = 0;
            return (false, true, vec![]);
        }
        (KeyCode::PageUp, _) if app.ui.focus == FocusPane::Diff => {
            app.diff.diff_scroll_offset = app.diff.diff_scroll_offset.saturating_sub(20);
            return (false, true, vec![]);
        }
        (KeyCode::PageDown, _) if app.ui.focus == FocusPane::Diff => {
            app.diff.diff_scroll_offset = app.diff.diff_scroll_offset.saturating_add(20);
            return (false, true, vec![]);
        }
        _ => {}
    }

    (false, false, vec![])
}

fn reduce_mouse(app: &mut AppState, mouse: crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};

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
                input.field.cursor = crate::text::edit::byte_index_at_display_col(
                    &input.field.buffer,
                    target_col,
                );
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
                app.exec.log_scroll_offset = app.exec.log_scroll_offset.saturating_add(LOG_WHEEL_STEP);
                return true;
            }
            if rect_contains(layout.diff_preview, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Preview;
                app.diff.diff_scroll_offset = app.diff.diff_scroll_offset.saturating_sub(DIFF_WHEEL_STEP);
                return true;
            }
            if rect_contains(layout.diff_files, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Files;
                select_adjacent_diff_file(app, -1);
                return true;
            }
            if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                app.ui.focus = FocusPane::Board;
                app.board.tasks_active_column = hit.status;
                ensure_selected_task_in_active_column(app);
                select_adjacent_task(app, -1);
                return true;
            }
        }
        MouseEventKind::ScrollDown => {
            if rect_contains(layout.exec_logs, col, row) {
                app.ui.focus = FocusPane::Execution;
                app.exec.log_scroll_offset = app.exec.log_scroll_offset.saturating_sub(LOG_WHEEL_STEP);
                if app.exec.log_scroll_offset == 0 {
                    app.exec.log_autoscroll = true;
                }
                return true;
            }
            if rect_contains(layout.diff_preview, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Preview;
                app.diff.diff_scroll_offset = app.diff.diff_scroll_offset.saturating_add(DIFF_WHEEL_STEP);
                return true;
            }
            if rect_contains(layout.diff_files, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Files;
                select_adjacent_diff_file(app, 1);
                return true;
            }
            if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                app.ui.focus = FocusPane::Board;
                app.board.tasks_active_column = hit.status;
                ensure_selected_task_in_active_column(app);
                select_adjacent_task(app, 1);
                return true;
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if rect_contains(layout.board, col, row) {
                app.ui.focus = FocusPane::Board;
                if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                    app.board.tasks_active_column = hit.status;
                    if let Some(idx) = hit.clicked_index {
                        app.board.board_index_by_status[hit.status.idx()] = idx;
                    }
                    if let Some(task_id) = hit.clicked_task_id {
                        set_selected_task(app, Some(task_id));
                    } else {
                        ensure_selected_task_in_active_column(app);
                    }
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
                            let line_w = crate::text::display_width(line_str);
                            target_col = target_col.min(line_w);

                            let within =
                                crate::text::edit::byte_index_at_display_col(line_str, target_col);
                            app.ui.composer.cursor = (ls + within).min(app.ui.composer.buffer.len());
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
                    app.exec.log_selected = log_entry_hit_at(app, layout.exec_logs, col, row);
                    return true;
                }
                return false;
            }

            if rect_contains(layout.diff, col, row) {
                app.ui.focus = FocusPane::Diff;
                if rect_contains(layout.diff_repo_bar, col, row) {
                    if let Some(action) = diff_repo_bar_action_at(app, layout.diff_repo_bar, col, row) {
                        trigger_diff_repo_action(app, action);
                    }
                    return true;
                }
                if rect_contains(layout.diff_files, col, row) {
                    app.ui.diff_focus = DiffFocus::Files;
                    if let Some(idx) = diff_files_hit_at(app, layout.diff_files, col, row) {
                        if idx != app.diff.selected_diff_index {
                            app.diff.selected_diff_index = idx;
                            app.diff.diff_scroll_offset = 0;
                            sync_selected_repo_from_diff_selection(app);
                            crate::diff_preview::schedule_diff_preview_refresh(app, Duration::from_millis(0));
                        }
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
                app.exec.log_selected = log_entry_hit_at(app, layout.exec_logs, col, row);
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

fn log_entry_hit_at(app: &AppState, area: Rect, col: u16, row: u16) -> Option<crate::logs::LogSelection> {
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

fn reduce_copy(app: &mut AppState, target: CopyTarget) -> Vec<Effect> {
    let text = match target {
        CopyTarget::Execution => {
            if let Some(sel) = app.exec.log_selected {
                app.exec
                    .log_buffers
                    .get(&sel.exec_id)
                    .and_then(|b| b.rendered_entry_text(sel.entry_idx))
            } else {
                None
            }
            .unwrap_or_else(|| {
                let layout = compute_main_layout(crate::layout::current_terminal_rect());
                let area = layout.exec_logs;
                let len = app.exec.log_lines.len();
                let max_render = area.height.saturating_sub(2) as usize;
                let visible = max_render.min(len).max(1);
                let mut offset = if app.exec.log_autoscroll {
                    0
                } else {
                    app.exec.log_scroll_offset
                };
                offset = offset.min(len.saturating_sub(visible));
                let start = len.saturating_sub(visible + offset);
                let end = len.saturating_sub(offset);
                crate::util::lines_plain_text(app.exec.log_lines.get(start..end).unwrap_or(&[]))
            })
        }
        CopyTarget::DiffFiles => {
            let rows = crate::diff::diff_rows_with_all(&app.diff.diff_store);
            rows.get(app.diff.selected_diff_index)
                .map(|d| d.key.clone())
                .unwrap_or_default()
        }
        CopyTarget::DiffPreview => crate::util::lines_plain_text(&app.diff.diff_preview_lines),
    };

    if text.trim().is_empty() {
        return vec![Effect::Toast {
            message: "Copy: nothing to copy".to_string(),
            color: Color::Yellow,
            expires_at: Some(Instant::now() + std::time::Duration::from_secs(2)),
        }];
    }

    let label = match target {
        CopyTarget::Execution => "Copied logs",
        CopyTarget::DiffFiles => "Copied path",
        CopyTarget::DiffPreview => "Copied diff",
    };

    vec![
        Effect::CopyOsc52(text),
        Effect::Toast {
            message: label.to_string(),
            color: Color::Green,
            expires_at: Some(Instant::now() + std::time::Duration::from_secs(2)),
        },
    ]
}

fn reduce_composer_key(
    app: &mut AppState,
    key: crossterm::event::KeyEvent,
) -> Option<(bool, Vec<Effect>)> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            app.ui.composer.undo();
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL)
        | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
            app.ui.composer.redo();
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            app.ui.composer.clear();
        }
        (KeyCode::Enter, KeyModifiers::CONTROL) => {
            app.ui.composer.insert_char('\n');
        }
        (KeyCode::Left, KeyModifiers::CONTROL) => {
            app.ui.composer.move_word_left();
        }
        (KeyCode::Right, KeyModifiers::CONTROL) => {
            app.ui.composer.move_word_right();
        }
        (KeyCode::Left, _) => {
            app.ui.composer.move_left();
        }
        (KeyCode::Right, _) => {
            app.ui.composer.move_right();
        }
        (KeyCode::Up, _) => {
            if app.ui.composer.buffer.trim_start().starts_with('/')
                && {
                    app.ui.composer.clamp_cursor();
                    app.ui.composer.cursor == app.ui.composer.buffer.len()
                }
            {
                crate::ui::move_composer_autocomplete(app, -1);
            } else {
                app.ui.composer.move_up();
            }
        }
        (KeyCode::Down, _) => {
            if app.ui.composer.buffer.trim_start().starts_with('/')
                && {
                    app.ui.composer.clamp_cursor();
                    app.ui.composer.cursor == app.ui.composer.buffer.len()
                }
            {
                crate::ui::move_composer_autocomplete(app, 1);
            } else {
                app.ui.composer.move_down();
            }
        }
        (KeyCode::Home, _) => {
            app.ui.composer.move_home(true);
        }
        (KeyCode::End, _) => {
            app.ui.composer.move_end(true);
        }
        (KeyCode::Backspace, KeyModifiers::ALT) | (KeyCode::Backspace, KeyModifiers::CONTROL) => {
            app.ui.composer.backspace_word();
        }
        (KeyCode::Backspace, _) => {
            app.ui.composer.backspace();
        }
        (KeyCode::Delete, KeyModifiers::CONTROL) => {
            app.ui.composer.delete_word();
        }
        (KeyCode::Delete, _) => {
            app.ui.composer.delete();
        }
        (KeyCode::Tab, _) => {
            crate::ui::apply_composer_autocomplete(app);
        }
        (KeyCode::Char(c), KeyModifiers::NONE) => {
            app.ui.composer.insert_char(c);
        }
        _ => return None,
    }

    app.ui.composer_suggest_index = 0;
    let layout = compute_main_layout(crate::layout::current_terminal_rect());
    let area = layout.exec_input;
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2) as usize;
    let prefix_w = crate::text::display_width("  ");
    let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);
    app.ui.composer.ensure_cursor_visible(content_w, inner_h.max(1));
    Some((true, vec![]))
}

fn reduce_search_key(
    app: &mut AppState,
    key: crossterm::event::KeyEvent,
) -> Option<(bool, Vec<Effect>)> {
    if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
        return None;
    }

    let Some(mut input) = app.ui.input.take() else {
        return None;
    };

    match (key.code, key.modifiers) {
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            input.field.undo();
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL)
        | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
            input.field.redo();
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            input.field.clear();
        }
        (KeyCode::Left, KeyModifiers::CONTROL) => {
            input.field.move_word_left();
        }
        (KeyCode::Right, KeyModifiers::CONTROL) => {
            input.field.move_word_right();
        }
        (KeyCode::Left, _) => {
            input.field.move_left();
        }
        (KeyCode::Right, _) => {
            input.field.move_right();
        }
        (KeyCode::Home, _) => {
            input.field.move_home(false);
        }
        (KeyCode::End, _) => {
            input.field.move_end(false);
        }
        (KeyCode::Backspace, KeyModifiers::ALT) | (KeyCode::Backspace, KeyModifiers::CONTROL) => {
            input.field.backspace_word();
        }
        (KeyCode::Backspace, _) => {
            input.field.backspace();
        }
        (KeyCode::Delete, KeyModifiers::CONTROL) => {
            input.field.delete_word();
        }
        (KeyCode::Delete, _) => {
            input.field.delete();
        }
        (KeyCode::Char(c), KeyModifiers::NONE) => {
            input.field.insert_char(c);
        }
        _ => return None,
    }

    app.board.task_filter = input.field.buffer.clone();
    let term = crate::layout::current_terminal_rect();
    let area = crate::ui::layout::centered_rect(80, 25, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1);
    input.field.ensure_cursor_visible(content_w, 1);
    crate::selection::ensure_selection_visible(app);

    app.ui.input = Some(input);
    Some((true, vec![]))
}

fn reduce_create_task_key(app: &mut AppState, key: crossterm::event::KeyEvent) {
    use crate::state::{CreateTaskFocus, TaskStatus};

    let Some(mut state) = app.ui.create_task.take() else {
        return;
    };
    state.error = None;

    fn status_cycle(status: TaskStatus, dir: i32) -> TaskStatus {
        let all = [
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::InReview,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ];
        let idx = all.iter().position(|s| *s == status).unwrap_or(0);
        if dir < 0 {
            all[idx.checked_sub(1).unwrap_or(0)]
        } else {
            all.get(idx + 1).copied().unwrap_or(all[all.len() - 1])
        }
    }

    fn next_focus(f: CreateTaskFocus) -> CreateTaskFocus {
        match f {
            CreateTaskFocus::Title => CreateTaskFocus::Description,
            CreateTaskFocus::Description => CreateTaskFocus::Status,
            CreateTaskFocus::Status => CreateTaskFocus::Buttons,
            CreateTaskFocus::Buttons => CreateTaskFocus::Title,
        }
    }

    fn prev_focus(f: CreateTaskFocus) -> CreateTaskFocus {
        match f {
            CreateTaskFocus::Title => CreateTaskFocus::Buttons,
            CreateTaskFocus::Description => CreateTaskFocus::Title,
            CreateTaskFocus::Status => CreateTaskFocus::Description,
            CreateTaskFocus::Buttons => CreateTaskFocus::Status,
        }
    }

    fn create_task_modal_desc_inner_dims(term: Rect) -> (usize, usize) {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::widgets::{Block, Borders};
        let area = crate::ui::layout::centered_rect(75, 70, term);
        let inner = Block::default().borders(Borders::ALL).inner(area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(7),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(inner);
        let desc = chunks[1];
        let inner_w = desc.width.saturating_sub(2) as usize;
        let inner_h = desc.height.saturating_sub(2) as usize;
        (inner_w.max(1), inner_h.max(1))
    }

    fn create_task_modal_title_inner_w(term: Rect) -> usize {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::widgets::{Block, Borders};
        let area = crate::ui::layout::centered_rect(75, 70, term);
        let inner = Block::default().borders(Borders::ALL).inner(area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(7),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(inner);
        let title = chunks[0];
        title.width.saturating_sub(2) as usize
    }

    let can_create = !state.title.buffer.trim().is_empty() && app.board.selected_project_id.is_some();
    let mut close = false;
    let mut submit = false;

    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => close = true,
        (KeyCode::Tab, KeyModifiers::SHIFT) => state.focus = prev_focus(state.focus),
        (KeyCode::Tab, _) => state.focus = next_focus(state.focus),
        (KeyCode::Enter, KeyModifiers::CONTROL) => {
            if can_create {
                submit = true;
            } else if app.board.selected_project_id.is_none() {
                state.error = Some("Project is required.".to_string());
            } else {
                state.error = Some("Title is required.".to_string());
            }
        }
        _ => match state.focus {
            CreateTaskFocus::Title => match (key.code, key.modifiers) {
                (KeyCode::Enter, _) => state.focus = CreateTaskFocus::Description,
                (KeyCode::Char('z'), KeyModifiers::CONTROL) => { state.title.undo(); }
                (KeyCode::Char('y'), KeyModifiers::CONTROL)
                | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => { state.title.redo(); }
                (KeyCode::Left, KeyModifiers::CONTROL) => state.title.move_word_left(),
                (KeyCode::Right, KeyModifiers::CONTROL) => state.title.move_word_right(),
                (KeyCode::Left, _) => state.title.move_left(),
                (KeyCode::Right, _) => state.title.move_right(),
                (KeyCode::Home, _) => state.title.move_home(false),
                (KeyCode::End, _) => state.title.move_end(false),
                (KeyCode::Backspace, KeyModifiers::ALT)
                | (KeyCode::Backspace, KeyModifiers::CONTROL) => state.title.backspace_word(),
                (KeyCode::Backspace, _) => state.title.backspace(),
                (KeyCode::Delete, KeyModifiers::CONTROL) => state.title.delete_word(),
                (KeyCode::Delete, _) => state.title.delete(),
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => state.title.clear(),
                (KeyCode::Char(c), KeyModifiers::NONE) => state.title.insert_char(c),
                _ => {}
            },
            CreateTaskFocus::Description => match (key.code, key.modifiers) {
                (KeyCode::Char('z'), KeyModifiers::CONTROL) => { state.description.undo(); }
                (KeyCode::Char('y'), KeyModifiers::CONTROL)
                | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => { state.description.redo(); }
                (KeyCode::Left, KeyModifiers::CONTROL) => state.description.move_word_left(),
                (KeyCode::Right, KeyModifiers::CONTROL) => state.description.move_word_right(),
                (KeyCode::Backspace, KeyModifiers::ALT)
                | (KeyCode::Backspace, KeyModifiers::CONTROL) => state.description.backspace_word(),
                (KeyCode::Delete, KeyModifiers::CONTROL) => state.description.delete_word(),
                (KeyCode::Backspace, _) => state.description.backspace(),
                (KeyCode::Delete, _) => state.description.delete(),
                (KeyCode::Left, _) => state.description.move_left(),
                (KeyCode::Right, _) => state.description.move_right(),
                (KeyCode::Up, _) => state.description.move_up(),
                (KeyCode::Down, _) => state.description.move_down(),
                (KeyCode::Home, _) => state.description.move_home(true),
                (KeyCode::End, _) => state.description.move_end(true),
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => state.description.clear(),
                (KeyCode::Enter, _) => state.description.insert_char('\n'),
                (KeyCode::Char(c), KeyModifiers::NONE) => state.description.insert_char(c),
                _ => {}
            },
            CreateTaskFocus::Status => match (key.code, key.modifiers) {
                (KeyCode::Left, _) | (KeyCode::Char('h'), _) => state.status = status_cycle(state.status, -1),
                (KeyCode::Right, _) | (KeyCode::Char('l'), _) => state.status = status_cycle(state.status, 1),
                (KeyCode::Up, _) | (KeyCode::Char('k'), _) => state.status = status_cycle(state.status, -1),
                (KeyCode::Down, _) | (KeyCode::Char('j'), _) => state.status = status_cycle(state.status, 1),
                (KeyCode::Enter, _) => state.focus = CreateTaskFocus::Buttons,
                _ => {}
            },
            CreateTaskFocus::Buttons => match (key.code, key.modifiers) {
                (KeyCode::Left, _) | (KeyCode::Char('h'), _) => state.selected_button = 0,
                (KeyCode::Right, _) | (KeyCode::Char('l'), _) => state.selected_button = 1,
                (KeyCode::Enter, _) => {
                    if state.selected_button == 1 {
                        close = true;
                    } else if can_create {
                        submit = true;
                    } else {
                        state.error = if app.board.selected_project_id.is_none() {
                            Some("Project is required.".to_string())
                        } else {
                            Some("Title is required.".to_string())
                        };
                    }
                }
                _ => {}
            },
        },
    }

    if state.focus == CreateTaskFocus::Description {
        let (inner_w, inner_h) = create_task_modal_desc_inner_dims(crate::layout::current_terminal_rect());
        state.description.ensure_cursor_visible(inner_w, inner_h);
    } else if state.focus == CreateTaskFocus::Title {
        let inner_w = create_task_modal_title_inner_w(crate::layout::current_terminal_rect());
        state.title.ensure_cursor_visible(inner_w.max(1), 1);
    }

    if close {
        return;
    }

    if submit {
        submit_create_task_state(app, state);
        return;
    }

    app.ui.create_task = Some(state);
}

fn submit_create_task_state(app: &mut AppState, state: crate::state::CreateTaskState) {
    let Some(project_id) = app.board.selected_project_id else {
        app.ui.create_task = Some(crate::state::CreateTaskState {
            error: Some("No project selected.".to_string()),
            ..state
        });
        return;
    };
    let title = state.title.buffer.trim().to_string();
    if title.is_empty() {
        app.ui.create_task = Some(crate::state::CreateTaskState {
            error: Some("Title is required.".to_string()),
            ..state
        });
        return;
    }
    let description = state
        .description
        .buffer
        .trim_end_matches('\n')
        .trim()
        .to_string();
    let description = (!description.is_empty()).then_some(description);
    let status = state.status;

    set_toast(
        app,
        "Creating task…".to_string(),
        Color::Cyan,
        Some(Instant::now() + std::time::Duration::from_secs(2)),
    );

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match crate::net::ops::create_task_http(&base_url, project_id, &title, description.as_deref(), status)
            .await
        {
            Ok(task_id) => {
                let _ = net_tx
                    .send(crate::events::NetEvent::TaskCreated { task_id, status })
                    .await;
                let _ = net_tx
                    .send(crate::events::NetEvent::Notice(format!("Created task: {title}")))
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(crate::events::NetEvent::Error(format!("create task failed: {e}")))
                    .await;
            }
        }
    });
}
