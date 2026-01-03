use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::commands::submit_composer;
use crate::layout::{compute_main_layout, current_terminal_rect};
use crate::prefs::save_prefs;
use crate::state::{AppState, ConfirmAction, ConfirmState, DiffFocus, FocusPane, LogMode, LogRenderMode, LogViewMode};
use crate::ui::{open_create_task_modal, trigger_diff_repo_action, DiffRepoAction};

use super::copy::reduce_copy;
use super::create_task;
use super::focus;
use super::modals;
use super::scroll;
use super::{CopyTarget, Effect};
use super::{sel};

pub(super) fn reduce_key(app: &mut AppState, key: KeyEvent) -> (bool, bool, Vec<Effect>) {
    // Confirm modal has highest priority.
    if let Some(confirm) = app.ui.confirm.as_ref() {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                let action = confirm.action;
                modals::close_confirm(app);
                crate::handle_confirm_action(app, action);
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                modals::close_confirm(app);
            }
            _ => {}
        }
        return (false, true, vec![]);
    }

    // Search modal.
    if app.ui.input.is_some() {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                modals::close_search(app, true);
                return (false, true, vec![]);
            }
            (KeyCode::Enter, _) => {
                modals::close_search_keep(app);
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
                modals::close_help(app);
                return (false, true, vec![]);
            }
            _ => return (false, false, vec![]),
        }
    }

    // Create-task modal.
    if app.ui.create_task.is_some() {
        create_task::handle_create_task_key(app, key);
        return (false, true, vec![]);
    }

    // Composer editing.
    if app.ui.composer_active {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                modals::close_composer(app);
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
            modals::open_help(app);
            return (false, true, vec![]);
        }
        (KeyCode::Tab, KeyModifiers::NONE) => {
            focus::cycle_focus(app);
            return (false, true, vec![]);
        }
        (KeyCode::Char('/'), _) => {
            modals::open_search(app);
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
            modals::open_composer(app);
            return (false, true, vec![]);
        }
        (KeyCode::Char('x'), _) => {
            if let Some(exec_id) = app.exec.selected_exec_id {
                modals::open_confirm(
                    app,
                    ConfirmState {
                    title: "Stop execution?".to_string(),
                    body: format!("Stop execution process {exec_id}? (y/n)"),
                    action: ConfirmAction::StopExec { exec_id },
                },
                );
                return (false, true, vec![]);
            }
        }
        (KeyCode::Char('['), _) => {
            sel::select_adjacent_attempt(app, -1);
            return (false, true, vec![]);
        }
        (KeyCode::Char(']'), _) => {
            sel::select_adjacent_attempt(app, 1);
            return (false, true, vec![]);
        }
        (KeyCode::Char('h'), _) if app.ui.focus == FocusPane::Diff => {
            focus::focus_diff_files(app);
            return (false, true, vec![]);
        }
        (KeyCode::Char('l'), _) if app.ui.focus == FocusPane::Diff => {
            focus::focus_diff_preview(app);
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
            sel::normalize_after_cancelled_toggle(app);
            app.prefs.show_cancelled = app.board.show_cancelled;
            save_prefs(&app.prefs);
            return (false, true, vec![]);
        }
        (KeyCode::Char('n'), _) if app.ui.focus == FocusPane::Board => {
            open_create_task_modal(app);
            return (false, true, vec![]);
        }
        (KeyCode::Char('K'), _) if app.ui.focus == FocusPane::Board => {
            sel::move_active_status(app, -1);
            return (false, true, vec![]);
        }
        (KeyCode::Char('J'), _) if app.ui.focus == FocusPane::Board => {
            sel::move_active_status(app, 1);
            return (false, true, vec![]);
        }
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) if app.ui.focus == FocusPane::Board => {
            sel::select_adjacent_task(app, -1);
            return (false, true, vec![]);
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) if app.ui.focus == FocusPane::Board => {
            sel::select_adjacent_task(app, 1);
            return (false, true, vec![]);
        }
        (KeyCode::Left, _) if app.ui.focus == FocusPane::Board => {
            sel::request_move_selected_task(app, -1);
            return (false, true, vec![]);
        }
        (KeyCode::Right, _) if app.ui.focus == FocusPane::Board => {
            sel::request_move_selected_task(app, 1);
            return (false, true, vec![]);
        }
        (KeyCode::Up, _) | (KeyCode::Char('k'), _)
            if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files =>
        {
            sel::select_adjacent_diff_file(app, -1);
            return (false, true, vec![]);
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _)
            if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files =>
        {
            sel::select_adjacent_diff_file(app, 1);
            return (false, true, vec![]);
        }
        (KeyCode::PageUp, _) if app.ui.focus == FocusPane::Execution => {
            scroll::scroll_exec_older(app, 40);
            return (false, true, vec![]);
        }
        (KeyCode::PageDown, _) if app.ui.focus == FocusPane::Execution => {
            scroll::scroll_exec_newer(app, 40);
            return (false, true, vec![]);
        }
        (KeyCode::End, _) if app.ui.focus == FocusPane::Execution => {
            scroll::scroll_exec_to_end(app);
            return (false, true, vec![]);
        }
        (KeyCode::PageUp, _) if app.ui.focus == FocusPane::Diff => {
            scroll::scroll_diff_up(app, 20);
            return (false, true, vec![]);
        }
        (KeyCode::PageDown, _) if app.ui.focus == FocusPane::Diff => {
            scroll::scroll_diff_down(app, 20);
            return (false, true, vec![]);
        }
        _ => {}
    }

    (false, false, vec![])
}

fn reduce_composer_key(app: &mut AppState, key: KeyEvent) -> Option<(bool, Vec<Effect>)> {
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
    let layout = compute_main_layout(current_terminal_rect());
    let area = layout.exec_input;
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2) as usize;
    let prefix_w = crate::text::display_width("  ");
    let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);
    app.ui.composer.ensure_cursor_visible(content_w, inner_h.max(1));
    Some((true, vec![]))
}

fn reduce_search_key(app: &mut AppState, key: KeyEvent) -> Option<(bool, Vec<Effect>)> {
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
    let term = current_terminal_rect();
    let area = crate::ui::layout::centered_rect(80, 25, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1);
    input.field.ensure_cursor_visible(content_w, 1);
    sel::ensure_selection_visible(app);

    app.ui.input = Some(input);
    Some((true, vec![]))
}
