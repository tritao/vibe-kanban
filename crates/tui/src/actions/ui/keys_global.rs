use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{focus, modals};
use crate::{
    prefs::save_prefs,
    state::{AppState, FocusPane, LogMode, LogRenderMode, LogViewMode},
};

pub(super) fn handle_global_key(app: &mut AppState, key: KeyEvent) -> Option<(bool, bool)> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => return Some((true, false)),
        (KeyCode::Char('?'), _) => {
            modals::open_help(app);
            return Some((false, true));
        }
        (KeyCode::Tab, KeyModifiers::NONE) => {
            focus::cycle_focus(app);
            return Some((false, true));
        }
        (KeyCode::Char('/'), _) => {
            modals::open_search(app);
            return Some((false, true));
        }
        (KeyCode::Char('r'), _) => {
            let next = *app.reconnect_tx.borrow() + 1;
            let _ = app.reconnect_tx.send(next);
            return Some((false, true));
        }
        (KeyCode::Char('i'), KeyModifiers::NONE) => {
            focus::focus_execution(app);
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
        (KeyCode::Char('m'), _) if app.ui.focus == FocusPane::Execution => {
            app.exec.log_render_mode = match app.exec.log_render_mode {
                LogRenderMode::Plain => LogRenderMode::Markdown,
                LogRenderMode::Markdown => LogRenderMode::Plain,
            };
            app.prefs.log_render_mode = app.exec.log_render_mode;
            save_prefs(&app.prefs);
            crate::logs::mark_all_log_buffers_dirty(app, 0);
            return Some((false, true));
        }
        (KeyCode::Char('v'), _) if app.ui.focus == FocusPane::Execution => {
            app.exec.log_view_mode = match app.exec.log_view_mode {
                LogViewMode::Timeline => LogViewMode::Single,
                LogViewMode::Single => LogViewMode::Timeline,
            };
            app.prefs.log_view_mode = app.exec.log_view_mode;
            save_prefs(&app.prefs);
            app.exec.log_view_dirty = true;
            return Some((false, true));
        }
        _ => {}
    }

    None
}
