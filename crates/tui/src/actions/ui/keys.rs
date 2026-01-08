use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{
    CopyTarget, Effect, composer, confirm, copy::reduce_copy, copy_targets, keys_global, modals,
    sel,
};
use crate::{
    commands::submit_composer,
    state::{AppState, FocusPane},
    ui::components,
};

pub(super) fn reduce_key(app: &mut AppState, key: KeyEvent) -> super::UiApplyResult {
    // Alt+S toggles mouse capture (enables terminal text selection).
    if matches!(
        (key.code, key.modifiers),
        (KeyCode::Char('s'), KeyModifiers::ALT)
    ) {
        return super::UiApplyResult::changed(false)
            .with_effect(Effect::SetMouseCapture(!app.ui.mouse_capture_enabled));
    }

    if let Some(res) = crate::ui::modals::reduce_modal_key(app, key) {
        return super::UiApplyResult {
            changed: res.dirty,
            should_quit: res.quit,
            effects: vec![],
        };
    }

    // Composer editing.
    if app.ui.composer_active {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                modals::close_composer(app);
                return super::UiApplyResult::changed(true);
            }
            (KeyCode::Enter, _) => {
                if crate::slash::apply_composer_autocomplete(app) {
                    return super::UiApplyResult::changed(true);
                }
                let quit = submit_composer(app);
                return super::UiApplyResult {
                    changed: true,
                    should_quit: quit,
                    effects: vec![],
                };
            }
            _ => {}
        }
        if composer::handle_composer_key(app, key) {
            return super::UiApplyResult::changed(true);
        }
        return super::UiApplyResult::none();
    }

    // Keymap dispatch.
    if let Some((quit, dirty)) = keys_global::handle_global_key(app, key) {
        return super::UiApplyResult {
            changed: dirty,
            should_quit: quit,
            effects: vec![],
        };
    }

    // Common selection shortcuts (independent of focus).
    match key.code {
        KeyCode::Char('[') => {
            sel::select_adjacent_attempt(app, -1);
            return super::UiApplyResult::changed(true);
        }
        KeyCode::Char(']') => {
            sel::select_adjacent_attempt(app, 1);
            return super::UiApplyResult::changed(true);
        }
        KeyCode::Char('x') => {
            if confirm::open_stop_exec_confirm(app) {
                return super::UiApplyResult::changed(true);
            }
        }
        _ => {}
    }

    // Shift+Y copies the worktree/repo root path (independent of focus).
    if key.code == KeyCode::Char('Y') {
        return super::UiApplyResult::none()
            .with_effects(reduce_copy(app, CopyTarget::WorktreePath));
    }

    // Shift+W copies the selected attempt's checkout path (for the selected repo).
    if key.code == KeyCode::Char('W') {
        return super::UiApplyResult::none()
            .with_effects(reduce_copy(app, CopyTarget::AttemptCheckoutPath));
    }

    if key.code == KeyCode::Char('y') {
        if let Some(target) = copy_targets::copy_target_for_focused_pane(app) {
            return super::UiApplyResult::none().with_effects(reduce_copy(app, target));
        }
    }

    let handled = match app.ui.focus {
        FocusPane::Board => components::handle_board_key(app, key),
        FocusPane::Diff => components::handle_diff_key(app, key),
        FocusPane::Execution => components::handle_exec_key(app, key),
    };
    if handled {
        return super::UiApplyResult::changed(true);
    }

    super::UiApplyResult::none()
}
