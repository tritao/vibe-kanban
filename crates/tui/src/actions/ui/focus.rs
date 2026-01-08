use crate::state::AppState;

pub(super) fn focus_execution(app: &mut AppState) {
    app.ui.focus_execution();
}

pub(super) fn cycle_focus(app: &mut AppState) {
    app.ui.cycle_focus();
}
