use crate::{
    events::StreamStatus,
    state::{UiMessageKey, app_state::UiState},
};

pub(super) fn apply_status(field: &mut StreamStatus, status: StreamStatus) -> bool {
    let changed = *field != status;
    *field = status;
    changed
}

pub(super) fn apply_status_clear_error_on_connected(
    ui: &mut UiState,
    field: &mut StreamStatus,
    status: StreamStatus,
    key: UiMessageKey,
) -> bool {
    let changed = apply_status(field, status);
    if matches!(status, StreamStatus::Connected | StreamStatus::Completed) {
        if ui.clear_error_scope(key) {
            return true;
        }
    }
    changed
}
