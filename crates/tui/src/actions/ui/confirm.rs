use super::modals;
use crate::state::{AppState, ConfirmAction, ConfirmState};

pub(super) fn handle_confirm_key(app: &mut AppState, key: crossterm::event::KeyEvent) -> bool {
    let Some(confirm) = app.ui.confirm.as_ref() else {
        return false;
    };

    match key.code {
        crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Enter => {
            let action = confirm.action;
            modals::close_confirm(app);
            crate::handle_confirm_action(app, action);
            true
        }
        crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Esc => {
            modals::close_confirm(app);
            true
        }
        _ => false,
    }
}

pub(super) fn open_stop_exec_confirm(app: &mut AppState) -> bool {
    let Some(exec_id) = app.exec.selected_exec_id else {
        return false;
    };

    modals::open_confirm(
        app,
        ConfirmState {
            title: "Stop execution?".to_string(),
            body: format!("Stop execution process {exec_id}? (y/n)"),
            action: ConfirmAction::StopExec { exec_id },
        },
    );
    true
}
