use crate::state::{AppState, ConfirmState};

pub(super) fn open_confirm(app: &mut AppState, state: ConfirmState) {
    app.ui.confirm = Some(state);
}

pub(super) fn open_composer(app: &mut AppState) {
    crate::ui::open_composer(app);
}

pub(super) fn close_composer(app: &mut AppState) {
    crate::ui::close_composer(app);
}
