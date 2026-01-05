use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::state::TextFieldState;

pub(super) fn apply_text_field_key(
    field: &mut TextFieldState,
    key: KeyEvent,
    multiline: bool,
) -> bool {
    match (key.code, key.modifiers) {
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            field.undo();
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL)
        | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
            field.redo();
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            field.clear();
        }
        (KeyCode::Left, KeyModifiers::CONTROL) => {
            field.move_word_left();
        }
        (KeyCode::Right, KeyModifiers::CONTROL) => {
            field.move_word_right();
        }
        (KeyCode::Left, _) => {
            field.move_left();
        }
        (KeyCode::Right, _) => {
            field.move_right();
        }
        (KeyCode::Up, _) if multiline => {
            field.move_up();
        }
        (KeyCode::Down, _) if multiline => {
            field.move_down();
        }
        (KeyCode::Home, _) => {
            field.move_home(multiline);
        }
        (KeyCode::End, _) => {
            field.move_end(multiline);
        }
        (KeyCode::Backspace, KeyModifiers::ALT) | (KeyCode::Backspace, KeyModifiers::CONTROL) => {
            field.backspace_word();
        }
        (KeyCode::Backspace, _) => {
            field.backspace();
        }
        (KeyCode::Delete, KeyModifiers::CONTROL) => {
            field.delete_word();
        }
        (KeyCode::Delete, _) => {
            field.delete();
        }
        (KeyCode::Enter, _) if multiline => {
            field.insert_char('\n');
        }
        (KeyCode::Char(c), KeyModifiers::NONE) => {
            field.insert_char(c);
        }
        _ => return false,
    }

    true
}
