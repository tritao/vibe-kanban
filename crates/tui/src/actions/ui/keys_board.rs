use crossterm::event::{KeyCode, KeyEvent};

use super::sel;
use crate::{
    prefs::save_prefs,
    state::{AppState, FocusPane},
    ui::open_create_task_modal,
};

pub(super) fn handle_board_key(app: &mut AppState, key: KeyEvent) -> Option<bool> {
    if app.ui.focus != FocusPane::Board {
        return None;
    }

    match key.code {
        KeyCode::Char('c') => {
            app.board.show_cancelled = !app.board.show_cancelled;
            sel::normalize_after_cancelled_toggle(app);
            app.prefs.show_cancelled = app.board.show_cancelled;
            save_prefs(&app.prefs);
            Some(true)
        }
        KeyCode::Char('n') => {
            open_create_task_modal(app, None);
            Some(true)
        }
        KeyCode::Char('N') => {
            open_create_task_modal(app, app.board.selected_task_id);
            Some(true)
        }
        KeyCode::Char('K') => {
            sel::move_active_status(app, -1);
            Some(true)
        }
        KeyCode::Char('J') => {
            sel::move_active_status(app, 1);
            Some(true)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            sel::select_adjacent_task(app, -1);
            Some(true)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            sel::select_adjacent_task(app, 1);
            Some(true)
        }
        KeyCode::Left => {
            sel::request_move_selected_task(app, -1);
            Some(true)
        }
        KeyCode::Right => {
            sel::request_move_selected_task(app, 1);
            Some(true)
        }
        _ => None,
    }
}
