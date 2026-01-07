use uuid::Uuid;

use crate::state::AppState;

pub(in crate::actions) fn select_project(app: &mut AppState, project_id: Option<Uuid>) {
    crate::selection::change::select_project(app, project_id);
}

pub(in crate::actions) fn select_task(app: &mut AppState, task_id: Option<Uuid>) {
    crate::selection::change::select_task(app, task_id);
}

pub(in crate::actions) fn select_attempt(app: &mut AppState, attempt_id: Option<Uuid>) {
    crate::selection::change::select_attempt(app, attempt_id);
}

pub(in crate::actions) fn select_exec(app: &mut AppState, exec_id: Option<Uuid>) {
    crate::selection::change::select_exec(app, exec_id);
}
