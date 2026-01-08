use crate::{
    actions::selection as sel,
    commands::{clear_pending_branch_status_refresh, on_exec_store_updated_for_branch_refresh},
    events::StreamStatus,
    state::AppState,
};

pub(super) fn exec_stream_status(app: &mut AppState, status: StreamStatus) -> bool {
    super::stream::apply_status(&mut app.exec.exec_status, status)
}

pub(super) fn exec_reset(app: &mut AppState) -> bool {
    app.exec.exec_store = serde_json::json!({ "execution_processes": {} });
    sel::select_exec(app, None);
    clear_pending_branch_status_refresh(app);
    true
}

pub(super) fn exec_patch(app: &mut AppState, patch: json_patch::Patch) -> bool {
    if let Err(e) = json_patch::patch(&mut app.exec.exec_store, &patch) {
        app.ui.set_error(format!("failed to apply exec patch: {e}"));
        app.exec.exec_status = StreamStatus::Error;
        return true;
    }
    sel::ensure_exec_selection(app);
    crate::logs::maybe_attach_pending_user_log(app);
    on_exec_store_updated_for_branch_refresh(app);
    true
}
