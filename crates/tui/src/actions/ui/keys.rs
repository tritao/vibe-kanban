use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::commands::submit_composer;
use crate::layout::{compute_main_layout, current_terminal_rect};
use crate::state::{AppState, ConfirmAction, ConfirmState, DiffFocus, FocusPane};

use super::copy::reduce_copy;
use super::create_task;
use super::keys_board;
use super::keys_diff;
use super::keys_exec;
use super::keys_global;
use super::modals;
use super::sel;
use super::{CopyTarget, Effect};

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

    // Keymap dispatch.
    if let Some((quit, dirty)) = keys_global::handle_global_key(app, key) {
        return (quit, dirty, vec![]);
    }

    // Common selection shortcuts (independent of focus).
    match key.code {
        KeyCode::Char('[') => {
            sel::select_adjacent_attempt(app, -1);
            return (false, true, vec![]);
        }
        KeyCode::Char(']') => {
            sel::select_adjacent_attempt(app, 1);
            return (false, true, vec![]);
        }
        KeyCode::Char('x') => {
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
        _ => {}
    }

    if key.code == KeyCode::Char('y') {
        let target = match app.ui.focus {
            FocusPane::Execution => Some(CopyTarget::Execution),
            FocusPane::Diff => Some(match app.ui.diff_focus {
                DiffFocus::Files => CopyTarget::DiffFiles,
                DiffFocus::Preview => CopyTarget::DiffPreview,
            }),
            _ => None,
        };
        if let Some(target) = target {
            return (false, false, reduce_copy(app, target));
        }
    }

    if let Some(dirty) = keys_board::handle_board_key(app, key) {
        return (false, dirty, vec![]);
    }
    if let Some(dirty) = keys_diff::handle_diff_key(app, key) {
        return (false, dirty, vec![]);
    }
    if let Some((dirty, effects)) = keys_exec::handle_exec_key(app, key) {
        return (false, dirty, effects);
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
