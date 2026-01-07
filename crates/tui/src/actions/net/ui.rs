use crate::state::{AppState, ExecutorProfileSelection, GitBranchItem};

pub(super) fn executor_profiles_loaded(
    app: &mut AppState,
    available: Vec<String>,
    selected: Option<ExecutorProfileSelection>,
    profiles_executors: serde_json::Value,
) -> bool {
    app.ui.available_executors = available;
    app.ui.selected_executor_profile = selected;
    app.ui.executor_profiles = profiles_executors;
    true
}

pub(super) fn repo_branches_loaded(
    app: &mut AppState,
    repo_id: uuid::Uuid,
    branches: Vec<GitBranchItem>,
) -> bool {
    if let Some(state) = app.ui.branch_picker.as_mut() {
        if state.repo_id == repo_id {
            state.branches = branches;
            state.busy = false;
            state.error = None;
            state.selected_index = state
                .selected_index
                .min(state.branches.len().saturating_sub(1));
            return true;
        }
    }
    false
}

pub(super) fn repo_branches_failed(
    app: &mut AppState,
    repo_id: uuid::Uuid,
    message: String,
) -> bool {
    if let Some(state) = app.ui.branch_picker.as_mut() {
        if state.repo_id == repo_id {
            state.busy = false;
            state.error = Some(message);
            return true;
        }
    }
    false
}

pub(super) fn notice(app: &mut AppState, msg: String) -> bool {
    app.ui.set_notice(msg);
    true
}

pub(super) fn error(app: &mut AppState, msg: String) -> bool {
    app.ui.set_error(msg);
    true
}

pub(super) fn error_key(app: &mut AppState, key: crate::state::UiMessageKey, msg: String) -> bool {
    app.ui.set_error_key(key, msg);
    true
}
