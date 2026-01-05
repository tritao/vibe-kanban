use crossterm::event::{KeyCode, KeyEvent};

use super::{Effect, modals, scroll};
use crate::state::{AppState, FocusPane};

pub(super) fn handle_exec_key(app: &mut AppState, key: KeyEvent) -> Option<(bool, Vec<Effect>)> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('e'), _) | (KeyCode::Enter, _) if app.ui.focus == FocusPane::Execution => {
            crate::logs::toggle_selected_log_entry(app);
            return Some((true, vec![]));
        }
        (KeyCode::Char('i'), _) if app.ui.focus == FocusPane::Execution => {
            modals::open_composer(app);
            return Some((true, vec![]));
        }
        (KeyCode::PageUp, _) if app.ui.focus == FocusPane::Execution => {
            scroll::scroll_exec_older(app, 40);
            return Some((true, vec![]));
        }
        (KeyCode::PageDown, _) if app.ui.focus == FocusPane::Execution => {
            scroll::scroll_exec_newer(app, 40);
            return Some((true, vec![]));
        }
        (KeyCode::End, _) if app.ui.focus == FocusPane::Execution => {
            scroll::scroll_exec_to_end(app);
            return Some((true, vec![]));
        }
        _ => {}
    }
    None
}
