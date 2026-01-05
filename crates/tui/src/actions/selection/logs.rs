use crate::{logs::LogSelection, state::AppState};

pub(in crate::actions) fn select_log_entry(app: &mut AppState, sel: Option<LogSelection>) {
    app.exec.log_selected = sel;
}
