use std::time::Duration;

use crate::commands::submit_composer;
use crate::diff::diff_rows_with_all;
use crate::diff_preview::schedule_diff_preview_refresh;
use crate::events::UiEvent;
use crate::layout::{compute_main_layout, current_terminal_rect, rect_contains};
use crate::logs::{
    mark_all_log_buffers_dirty, reset_logs, toggle_selected_log_entry, LogSelection,
};
use crate::selection::{
    ensure_selected_task_in_active_column, ensure_selection_visible, move_active_status,
    request_move_selected_task, select_adjacent_attempt, select_adjacent_diff_file,
    select_adjacent_task, set_selected_task,
};
use crate::state::{
    AppState, ConfirmAction, ConfirmState, DiffFocus, FocusPane, InputState, LogMode,
    LogRenderMode, LogViewMode, InputMode, TaskStatus,
};
use crate::ui::{
    apply_composer_autocomplete, board_hit_at, diff_repo_bar_action_at, handle_create_task_key,
    move_composer_autocomplete, open_create_task_modal, sync_selected_repo_from_diff_selection,
    trigger_diff_repo_action, DiffRepoAction,
};
use crate::util::window_for_list;
use crate::{handle_confirm_action, prefs::save_prefs};

pub(crate) fn handle_ui_event(app: &mut AppState, event: UiEvent) -> anyhow::Result<bool> {
    match event {
        UiEvent::Tick => Ok(false),
        UiEvent::Crossterm(ev) => match ev {
            crossterm::event::Event::Key(key) => {
                use crossterm::event::{KeyCode, KeyModifiers};

                if let Some(confirm) = app.ui.confirm.as_ref() {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Enter => {
                            let action = confirm.action;
                            app.ui.confirm = None;
                            handle_confirm_action(app, action);
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            app.ui.confirm = None;
                        }
                        _ => {}
                    }
                    return Ok(false);
                }

                if let Some(mut input) = app.ui.input.take() {
                    let mut close = false;
                    let mut filter_changed = false;

                    match (key.code, key.modifiers) {
                        (KeyCode::Esc, _) => {
                            app.board.task_filter = input.original.clone();
                            close = true;
                            filter_changed = true;
                        }
                        (KeyCode::Enter, _) => {
                            close = true;
                        }
                        (KeyCode::Backspace, KeyModifiers::ALT)
                        | (KeyCode::Backspace, KeyModifiers::CONTROL) => {
                            input.field.backspace_word();
                            app.board.task_filter = input.field.buffer.clone();
                            filter_changed = true;
                        }
                        (KeyCode::Backspace, _) => {
                            input.field.backspace();
                            app.board.task_filter = input.field.buffer.clone();
                            filter_changed = true;
                        }
                        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                            input.field.clear();
                            app.board.task_filter.clear();
                            filter_changed = true;
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
                        (KeyCode::Char(c), KeyModifiers::NONE) => {
                            input.field.insert_char(c);
                            app.board.task_filter = input.field.buffer.clone();
                            filter_changed = true;
                        }
                        _ => {}
                    }

                    // Keep the caret visible (single-line input).
                    let term = current_terminal_rect();
                    let area = crate::ui::layout::centered_rect(80, 25, term);
                    let inner_w = area.width.saturating_sub(2) as usize;
                    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1); // "/" + free cell
                    input.field.ensure_cursor_visible(content_w, 1);

                    if close {
                        if filter_changed {
                            ensure_selection_visible(app);
                        }
                    } else {
                        if filter_changed {
                            ensure_selection_visible(app);
                        }
                        app.ui.input = Some(input);
                    }
                    return Ok(false);
                }

                if app.ui.show_help {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc => {
                            app.ui.show_help = false;
                        }
                        _ => {}
                    }
                    return Ok(false);
                }

                if app.ui.create_task.is_some() {
                    handle_create_task_key(app, key);
                    return Ok(false);
                }

                if app.ui.composer_active {
                    match (key.code, key.modifiers) {
                        (KeyCode::Esc, _) => {
                            app.ui.composer_active = false;
                            app.ui.composer.clear();
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Enter, KeyModifiers::CONTROL) => {
                            app.ui.composer.insert_char('\n');
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Enter, _) => {
                            submit_composer(app);
                        }
                        (KeyCode::Tab, _) => {
                            if apply_composer_autocomplete(app) {
                                // keep composing
                            }
                        }
                        (KeyCode::Up, _) => {
                            if app.ui.composer.buffer.trim_start().starts_with('/')
                                && {
                                    app.ui.composer.clamp_cursor();
                                    app.ui.composer.cursor == app.ui.composer.buffer.len()
                                }
                            {
                                move_composer_autocomplete(app, -1);
                            } else {
                                app.ui.composer.move_up();
                                app.ui.composer_suggest_index = 0;
                            }
                        }
                        (KeyCode::Down, _) => {
                            if app.ui.composer.buffer.trim_start().starts_with('/')
                                && {
                                    app.ui.composer.clamp_cursor();
                                    app.ui.composer.cursor == app.ui.composer.buffer.len()
                                }
                            {
                                move_composer_autocomplete(app, 1);
                            } else {
                                app.ui.composer.move_down();
                                app.ui.composer_suggest_index = 0;
                            }
                        }
                        (KeyCode::Left, KeyModifiers::CONTROL) => {
                            app.ui.composer.move_word_left();
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Right, KeyModifiers::CONTROL) => {
                            app.ui.composer.move_word_right();
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Left, _) => {
                            app.ui.composer.move_left();
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Right, _) => {
                            app.ui.composer.move_right();
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Home, _) => {
                            app.ui.composer.move_home(true);
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::End, _) => {
                            app.ui.composer.move_end(true);
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Backspace, KeyModifiers::ALT)
                        | (KeyCode::Backspace, KeyModifiers::CONTROL) => {
                            app.ui.composer.backspace_word();
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Backspace, _) => {
                            app.ui.composer.backspace();
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Delete, KeyModifiers::CONTROL) => {
                            app.ui.composer.delete_word();
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Delete, _) => {
                            app.ui.composer.delete();
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                            app.ui.composer.clear();
                            app.ui.composer_suggest_index = 0;
                        }
                        (KeyCode::Char(c), KeyModifiers::NONE) => {
                            app.ui.composer.insert_char(c);
                            app.ui.composer_suggest_index = 0;
                        }
                        _ => {}
                    }

                    if app.ui.composer_active {
                        let layout = compute_main_layout(current_terminal_rect());
                        let area = layout.exec_input;
                        let inner_w = area.width.saturating_sub(2) as usize;
                        let inner_h = area.height.saturating_sub(2) as usize;
                        let prefix_w = crate::text::display_width("  ");
                        let content_w = inner_w
                            .saturating_sub(prefix_w)
                            .saturating_sub(1)
                            .max(1);
                        app.ui.composer.ensure_cursor_visible(content_w, inner_h.max(1));
                    }
                    return Ok(false);
                }

                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) => return Ok(true),
                    (KeyCode::Char('?'), _) => {
                        app.ui.show_help = true;
                    }
                    (KeyCode::Tab, KeyModifiers::NONE) => {
                        app.ui.focus = match app.ui.focus {
                            FocusPane::Board => FocusPane::Execution,
                            FocusPane::Execution => FocusPane::Diff,
                            FocusPane::Diff => FocusPane::Board,
                        };
                    }
                    (KeyCode::Char('/'), _) => {
                        let mut field = crate::state::TextFieldState {
                            buffer: app.board.task_filter.clone(),
                            ..Default::default()
                        };
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
                    }
                    (KeyCode::Char('r'), _) => {
                        let next = *app.reconnect_tx.borrow() + 1;
                        let _ = app.reconnect_tx.send(next);
                    }
                    (KeyCode::Char('o'), _) => {
                        app.exec.log_mode = match app.exec.log_mode {
                            LogMode::Normalized => LogMode::Raw,
                            LogMode::Raw => LogMode::Normalized,
                        };
                        reset_logs(app, None);
                        let _ = app.log_mode_tx.send(app.exec.log_mode);
                        app.prefs.log_mode = app.exec.log_mode;
                        save_prefs(&app.prefs);
                    }
                    (KeyCode::Char('m'), _) if app.ui.focus == FocusPane::Execution => {
                        app.exec.log_render_mode = match app.exec.log_render_mode {
                            LogRenderMode::Plain => LogRenderMode::Markdown,
                            LogRenderMode::Markdown => LogRenderMode::Plain,
                        };
                        app.prefs.log_render_mode = app.exec.log_render_mode;
                        save_prefs(&app.prefs);
                        mark_all_log_buffers_dirty(app, 0);
                    }
                    (KeyCode::Char('v'), _) if app.ui.focus == FocusPane::Execution => {
                        app.exec.log_view_mode = match app.exec.log_view_mode {
                            LogViewMode::Timeline => LogViewMode::Single,
                            LogViewMode::Single => LogViewMode::Timeline,
                        };
                        app.prefs.log_view_mode = app.exec.log_view_mode;
                        save_prefs(&app.prefs);
                        app.exec.log_view_dirty = true;
                    }
                    (KeyCode::Char('e'), _) | (KeyCode::Enter, _)
                        if app.ui.focus == FocusPane::Execution =>
                    {
                        toggle_selected_log_entry(app);
                    }
                    (KeyCode::Char('d'), _) => {
                        app.diff.diff_stats_only = !app.diff.diff_stats_only;
                        let _ = app.diff_stats_tx.send(app.diff.diff_stats_only);
                        app.diff.diff_scroll_offset = 0;
                    }
                    (KeyCode::Char('t'), _) if app.ui.focus == FocusPane::Diff => {
                        app.diff.diff_theme = app.diff.diff_theme.cycle_next();
                        app.prefs.diff_theme = app.diff.diff_theme;
                        save_prefs(&app.prefs);
                        app.diff.diff_preview_cache_key = None;
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
                        let content_w = inner_w
                            .saturating_sub(prefix_w)
                            .saturating_sub(1)
                            .max(1);
                        app.ui.composer.ensure_cursor_visible(content_w, inner_h.max(1));
                    }
                    (KeyCode::Char('x'), _) => {
                        if let Some(exec_id) = app.exec.selected_exec_id {
                            app.ui.confirm = Some(ConfirmState {
                                title: "Stop execution?".to_string(),
                                body: format!("Stop execution process {exec_id}? (y/n)"),
                                action: ConfirmAction::StopExec { exec_id },
                            });
                        }
                    }
                    (KeyCode::Char('['), _) => {
                        select_adjacent_attempt(app, -1);
                    }
                    (KeyCode::Char(']'), _) => {
                        select_adjacent_attempt(app, 1);
                    }
                    (KeyCode::Char('h'), _) if app.ui.focus == FocusPane::Diff => {
                        app.ui.diff_focus = DiffFocus::Files;
                    }
                    (KeyCode::Char('l'), _) if app.ui.focus == FocusPane::Diff => {
                        app.ui.diff_focus = DiffFocus::Preview;
                    }
                    (KeyCode::Char('w'), _) if app.ui.focus == FocusPane::Diff => {
                        app.diff.diff_wrap = !app.diff.diff_wrap;
                        app.prefs.diff_wrap = app.diff.diff_wrap;
                        save_prefs(&app.prefs);
                        app.diff.diff_preview_cache_key = None;
                    }
                    (KeyCode::Char('S'), _) if app.ui.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::RefreshStatus);
                    }
                    (KeyCode::Char('M'), _) if app.ui.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::Merge);
                    }
                    (KeyCode::Char('R'), _) if app.ui.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::Rebase);
                    }
                    (KeyCode::Char('P'), _) if app.ui.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::CreatePr);
                    }
                    (KeyCode::Char('C'), _) if app.ui.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::ResolveConflicts);
                    }
                    (KeyCode::Char('O'), _) if app.ui.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::OpenConflict);
                    }
                    (KeyCode::Char('A'), _) if app.ui.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::AbortConflicts);
                    }
                    (KeyCode::Char('U'), _) if app.ui.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::OpenPr);
                    }
                    (KeyCode::Enter, _) if app.ui.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::OpenPr);
                    }
                    (KeyCode::Char('c'), _) if app.ui.focus == FocusPane::Board => {
                        app.board.show_cancelled = !app.board.show_cancelled;
                        if !app.board.show_cancelled
                            && app.board.tasks_active_column == TaskStatus::Cancelled
                        {
                            app.board.tasks_active_column = TaskStatus::Done;
                        }
                        app.prefs.show_cancelled = app.board.show_cancelled;
                        save_prefs(&app.prefs);
                    }
                    (KeyCode::Char('n'), _) if app.ui.focus == FocusPane::Board => {
                        open_create_task_modal(app);
                    }
                    (KeyCode::Char('K'), _) if app.ui.focus == FocusPane::Board => {
                        move_active_status(app, -1);
                    }
                    (KeyCode::Char('J'), _) if app.ui.focus == FocusPane::Board => {
                        move_active_status(app, 1);
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _)
                        if app.ui.focus == FocusPane::Board =>
                    {
                        select_adjacent_task(app, -1);
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _)
                        if app.ui.focus == FocusPane::Board =>
                    {
                        select_adjacent_task(app, 1);
                    }
                    (KeyCode::Left, _) if app.ui.focus == FocusPane::Board => {
                        request_move_selected_task(app, -1);
                    }
                    (KeyCode::Right, _) if app.ui.focus == FocusPane::Board => {
                        request_move_selected_task(app, 1);
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _)
                        if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files =>
                    {
                        select_adjacent_diff_file(app, -1);
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _)
                        if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files =>
                    {
                        select_adjacent_diff_file(app, 1);
                    }
                    (KeyCode::PageUp, _) if app.ui.focus == FocusPane::Execution => {
                        app.exec.log_autoscroll = false;
                        app.exec.log_scroll_offset = app.exec.log_scroll_offset.saturating_add(40);
                    }
                    (KeyCode::PageDown, _) if app.ui.focus == FocusPane::Execution => {
                        app.exec.log_scroll_offset = app.exec.log_scroll_offset.saturating_sub(40);
                        if app.exec.log_scroll_offset == 0 {
                            app.exec.log_autoscroll = true;
                        }
                    }
                    (KeyCode::End, _) if app.ui.focus == FocusPane::Execution => {
                        app.exec.log_autoscroll = true;
                        app.exec.log_scroll_offset = 0;
                    }
                    (KeyCode::PageUp, _) if app.ui.focus == FocusPane::Diff => {
                        app.diff.diff_scroll_offset = app.diff.diff_scroll_offset.saturating_sub(20);
                    }
                    (KeyCode::PageDown, _) if app.ui.focus == FocusPane::Diff => {
                        app.diff.diff_scroll_offset = app.diff.diff_scroll_offset.saturating_add(20);
                    }
                    _ => {}
                }
                Ok(false)
            }
            crossterm::event::Event::Mouse(mouse) => {
                handle_mouse_event(app, mouse);
                Ok(false)
            }
            _ => Ok(false),
        },
    }
}

fn diff_files_hit_at(app: &AppState, area: ratatui::layout::Rect, col: u16, row: u16) -> Option<usize> {
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
    area: ratatui::layout::Rect,
    col: u16,
    row: u16,
) -> Option<LogSelection> {
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

fn handle_mouse_event(app: &mut AppState, mouse: crossterm::event::MouseEvent) {
    if app.ui.confirm.is_some()
        || app.ui.input.is_some()
        || app.ui.show_help
        || app.ui.create_task.is_some()
    {
        return;
    }

    use crossterm::event::{MouseButton, MouseEventKind};

    let col = mouse.column;
    let row = mouse.row;
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
                return;
            }
            if rect_contains(layout.diff_preview, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Preview;
                app.diff.diff_scroll_offset =
                    app.diff.diff_scroll_offset.saturating_sub(DIFF_WHEEL_STEP);
                return;
            }
            if rect_contains(layout.diff_files, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Files;
                select_adjacent_diff_file(app, -1);
                return;
            }
            if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                app.ui.focus = FocusPane::Board;
                app.board.tasks_active_column = hit.status;
                ensure_selected_task_in_active_column(app);
                select_adjacent_task(app, -1);
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
                return;
            }
            if rect_contains(layout.diff_preview, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Preview;
                app.diff.diff_scroll_offset =
                    app.diff.diff_scroll_offset.saturating_add(DIFF_WHEEL_STEP);
                return;
            }
            if rect_contains(layout.diff_files, col, row) {
                app.ui.focus = FocusPane::Diff;
                app.ui.diff_focus = DiffFocus::Files;
                select_adjacent_diff_file(app, 1);
                return;
            }
            if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                app.ui.focus = FocusPane::Board;
                app.board.tasks_active_column = hit.status;
                ensure_selected_task_in_active_column(app);
                select_adjacent_task(app, 1);
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
                return;
            }

            if rect_contains(layout.exec, col, row) {
                app.ui.focus = FocusPane::Execution;
                if rect_contains(layout.exec_input, col, row) {
                    app.ui.composer_active = true;
                    app.ui.composer.set_end();
                    let area = layout.exec_input;
                    let inner_w = area.width.saturating_sub(2) as usize;
                    let inner_h = area.height.saturating_sub(2) as usize;
                    let prefix_w = crate::text::display_width("  ");
                    let content_w = inner_w
                        .saturating_sub(prefix_w)
                        .saturating_sub(1)
                        .max(1);
                    app.ui.composer.ensure_cursor_visible(content_w, inner_h.max(1));
                } else if rect_contains(layout.exec_logs, col, row) {
                    app.exec.log_selected = log_entry_hit_at(app, layout.exec_logs, col, row);
                }
                return;
            }

            if rect_contains(layout.diff, col, row) {
                app.ui.focus = FocusPane::Diff;
                if rect_contains(layout.diff_repo_bar, col, row) {
                    if let Some(action) = diff_repo_bar_action_at(app, layout.diff_repo_bar, col, row)
                    {
                        trigger_diff_repo_action(app, action);
                    }
                    return;
                }
                if rect_contains(layout.diff_files, col, row) {
                    app.ui.diff_focus = DiffFocus::Files;
                    if let Some(idx) = diff_files_hit_at(app, layout.diff_files, col, row) {
                        if idx != app.diff.selected_diff_index {
                            app.diff.selected_diff_index = idx;
                            app.diff.diff_scroll_offset = 0;
                            sync_selected_repo_from_diff_selection(app);
                            schedule_diff_preview_refresh(app, Duration::from_millis(0));
                        }
                    }
                } else if rect_contains(layout.diff_preview, col, row) {
                    app.ui.diff_focus = DiffFocus::Preview;
                }
                return;
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            if rect_contains(layout.exec_logs, col, row) {
                app.ui.focus = FocusPane::Execution;
                app.exec.log_selected = log_entry_hit_at(app, layout.exec_logs, col, row);
                toggle_selected_log_entry(app);
                return;
            }
        }
        _ => {}
    }
}
