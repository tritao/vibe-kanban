use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::layout::{compute_main_layout, current_terminal_rect};
use crate::state::AppState;

use super::text_edit;

pub(super) fn handle_composer_key(app: &mut AppState, key: KeyEvent) -> bool {
    // Autocomplete navigation when composing slash commands and cursor is at end.
    match key.code {
        KeyCode::Up | KeyCode::Down => {
            if app.ui.composer.buffer.trim_start().starts_with('/')
                && {
                    app.ui.composer.clamp_cursor();
                    app.ui.composer.cursor == app.ui.composer.buffer.len()
                }
            {
                let delta = if matches!(key.code, KeyCode::Up) { -1 } else { 1 };
                crate::ui::move_composer_autocomplete(app, delta);
                return true;
            }
        }
        _ => {}
    }

    match (key.code, key.modifiers) {
        // Ctrl-Enter inserts newline explicitly.
        (KeyCode::Enter, KeyModifiers::CONTROL) => {
            app.ui.composer.insert_char('\n');
        }
        // Tab applies autocomplete (only when suggestions active).
        (KeyCode::Tab, _) => {
            crate::ui::apply_composer_autocomplete(app);
        }
        _ => {
            if !text_edit::apply_text_field_key(&mut app.ui.composer, key, true) {
                return false;
            }
        }
    }

    app.ui.composer_suggest_index = 0;
    let layout = compute_main_layout(current_terminal_rect());
    let area = layout.exec_input;
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2) as usize;
    let prefix_w = crate::text::display_width("  ");
    let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);
    app.ui.composer.ensure_cursor_visible(content_w, inner_h.max(1));
    true
}

