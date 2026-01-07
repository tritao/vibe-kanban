use crate::{
    events::StreamStatus,
    logs::{enqueue_log_patch, reset_logs},
    state::AppState,
};

pub(super) fn log_stream_status(app: &mut AppState, status: StreamStatus) -> bool {
    app.exec.log_status = status;
    if matches!(status, StreamStatus::Connected | StreamStatus::Completed) {
        app.ui.clear_error_with_prefix("log stream connect:");
    }
    true
}

pub(super) fn log_reset(app: &mut AppState, exec_id: Option<uuid::Uuid>) -> bool {
    // Keep cached buffers unless we're explicitly resetting a specific exec buffer.
    match exec_id {
        Some(id) => reset_logs(app, Some(id)),
        None => crate::logs::reset_log_view(app, None),
    }
    true
}

pub(super) fn log_patch(app: &mut AppState, exec_id: uuid::Uuid, patch: json_patch::Patch) -> bool {
    enqueue_log_patch(app, app.board.selected_attempt_id, exec_id, patch);
    true
}

pub(super) fn log_prewarm_ready(
    app: &mut AppState,
    exec_id: uuid::Uuid,
    width: u16,
    generation: u64,
    cache: crate::logs::PreparedLogCache,
) -> bool {
    if generation != app.exec.log_prewarm_gen {
        return false;
    }
    if width != app.exec.log_target_render_width {
        return false;
    }
    if let Some(buf) = app.exec.log_buffers.get_mut(&exec_id) {
        buf.install_cache(width, cache);
    }
    false
}
