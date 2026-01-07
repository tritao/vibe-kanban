use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{
    CopyTarget, Effect, composer, confirm, copy::reduce_copy, keys_diff, keys_exec, keys_global,
    modals, sel,
};
use crate::{
    commands::submit_composer,
    state::{AppState, DiffFocus, FocusPane},
};

pub(super) fn reduce_key(app: &mut AppState, key: KeyEvent) -> (bool, bool, Vec<Effect>) {
    // Alt+S toggles mouse capture (enables terminal text selection).
    if matches!(
        (key.code, key.modifiers),
        (KeyCode::Char('s'), KeyModifiers::ALT)
    ) {
        return (
            false,
            true,
            vec![Effect::SetMouseCapture(!app.ui.mouse_capture_enabled)],
        );
    }

    if let Some(res) = crate::ui::modals::reduce_modal_key(app, key) {
        return (res.quit, res.dirty, vec![]);
    }

    // Composer editing.
    if app.ui.composer_active {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                modals::close_composer(app);
                return (false, true, vec![]);
            }
            (KeyCode::Enter, _) => {
                if crate::slash::apply_composer_autocomplete(app) {
                    return (false, true, vec![]);
                }
                let quit = submit_composer(app);
                return (quit, true, vec![]);
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

    // Shift+Y copies the worktree/repo root path (independent of focus).
    if key.code == KeyCode::Char('Y') {
        return (false, false, reduce_copy(app, CopyTarget::WorktreePath));
    }

    // Shift+W copies the selected attempt's checkout path (for the selected repo).
    if key.code == KeyCode::Char('W') {
        return (
            false,
            false,
            reduce_copy(app, CopyTarget::AttemptCheckoutPath),
        );
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

    if <crate::ui::components::board_pane::BoardPane as crate::ui::components::UiComponent>::on_event(
        app,
        crate::ui::components::board_pane::BoardPaneEvent::Key(key),
    ) {
        return (false, true, vec![]);
    }
    if let Some(dirty) = keys_diff::handle_diff_key(app, key) {
        return (false, dirty, vec![]);
    }
    if let Some((dirty, effects)) = keys_exec::handle_exec_key(app, key) {
        return (false, dirty, effects);
    }

    (false, false, vec![])
}
