use crate::state::{AppState, ConfirmAction, ConfirmState};

pub(super) fn open_stop_exec_confirm(app: &mut AppState) -> bool {
    let Some(exec_id) = app.exec.selected_exec_id else {
        return false;
    };

    super::modals::open_confirm(
        app,
        ConfirmState {
            title: "Stop execution?".to_string(),
            body: format!("Stop execution process {exec_id}? (y/n)"),
            action: ConfirmAction::StopExec { exec_id },
            alt_action: None,
        },
    );
    true
}
