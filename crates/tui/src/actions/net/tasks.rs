use crate::{
    actions::selection as sel,
    events::StreamStatus,
    state::{AppState, AttemptRow, TaskStatus},
};

pub(super) fn tasks_stream_status(app: &mut AppState, status: StreamStatus) -> bool {
    super::stream::apply_status(&mut app.board.tasks_status, status)
}

pub(super) fn tasks_reset(app: &mut AppState) -> bool {
    app.board.tasks_store = crate::store::tasks::empty_tasks_store();
    sel::select_task(app, None);
    app.board.pending_select_task_id = None;
    true
}

pub(super) fn tasks_patch(app: &mut AppState, patch: json_patch::Patch) -> bool {
    if let Err(e) = json_patch::patch(&mut app.board.tasks_store, &patch) {
        app.ui
            .set_error(format!("failed to apply tasks patch: {e}"));
        app.board.tasks_status = StreamStatus::Error;
        return true;
    }
    sel::reconcile_tasks_selection(app);
    true
}

pub(super) fn attempts_loaded(
    app: &mut AppState,
    task_id: uuid::Uuid,
    attempts: Vec<AttemptRow>,
) -> bool {
    if app.board.selected_task_id != Some(task_id) {
        return false;
    }

    sel::set_attempts(app, attempts);
    true
}

pub(super) fn task_created(app: &mut AppState, task_id: uuid::Uuid, status: TaskStatus) -> bool {
    // Place the new task in the expected column immediately, then select it when it appears.
    sel::note_task_created(app, task_id, status);
    // The tasks stream patch can arrive before the create-task HTTP call returns.
    // If the task is already present in the store, reconcile now so the new task is
    // selected immediately; otherwise `pending_select_task_id` will be picked up on the
    // next tasks patch.
    sel::reconcile_tasks_selection(app);
    true
}
