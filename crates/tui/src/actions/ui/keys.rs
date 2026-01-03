use crossterm::event::{KeyCode, KeyEvent};

use crate::commands::submit_composer;
use crate::layout::{current_terminal_rect};
use crate::state::{AppState, DiffFocus, FocusPane};

use super::confirm;
use super::copy::reduce_copy;
use super::composer;
use super::create_task;
use super::keys_board;
use super::keys_diff;
use super::keys_exec;
use super::keys_global;
use super::modals;
use super::sel;
use super::text_edit;
use super::{CopyTarget, Effect};

pub(super) fn reduce_key(app: &mut AppState, key: KeyEvent) -> (bool, bool, Vec<Effect>) {
    // Confirm modal has highest priority.
    if confirm::handle_confirm_key(app, key) {
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
        if composer::handle_composer_key(app, key) {
            return (false, true, vec![]);
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
            if confirm::open_stop_exec_confirm(app) {
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

fn reduce_search_key(app: &mut AppState, key: KeyEvent) -> Option<(bool, Vec<Effect>)> {
    if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
        return None;
    }

    let Some(mut input) = app.ui.input.take() else {
        return None;
    };

    if !text_edit::apply_text_field_key(&mut input.field, key, false) {
        app.ui.input = Some(input);
        return None;
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
