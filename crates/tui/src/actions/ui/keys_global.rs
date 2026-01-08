use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::modals;
use crate::{
    prefs::save_prefs,
    state::{AppState, LogMode},
};

pub(super) fn handle_global_key(app: &mut AppState, key: KeyEvent) -> Option<(bool, bool)> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => return Some((true, false)),
        (KeyCode::Char('?'), _) => {
            crate::ui::modals::open_help(app);
            return Some((false, true));
        }
        (KeyCode::Tab, KeyModifiers::NONE) => {
            app.ui.cycle_focus();
            return Some((false, true));
        }
        (KeyCode::Char('/'), _) => {
            crate::ui::modals::open_search(app);
            return Some((false, true));
        }
        (KeyCode::Char('r'), _) => {
            let next = *app.reconnect_tx.borrow() + 1;
            let _ = app.reconnect_tx.send(next);
            return Some((false, true));
        }
        (KeyCode::Char('i'), KeyModifiers::NONE) => {
            app.ui.focus_execution();
            modals::open_composer(app);
            return Some((false, true));
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
            return Some((false, true));
        }
        _ => {}
    }

    None
}
