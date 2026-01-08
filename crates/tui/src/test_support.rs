#![allow(dead_code)]

use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use crate::{
    events::NetEvent,
    state::{AppState, AttemptRow, LogMode, TaskStatus},
    store::roots::{DiffRoot, ProjectsRoot, TasksRoot},
};

pub(crate) fn mk_app() -> AppState {
    let (net_tx, _net_rx) = mpsc::channel::<NetEvent>(8);
    let (project_sel_tx, _project_sel_rx) = watch::channel(None);
    let (attempt_sel_tx, _attempt_sel_rx) = watch::channel(None);
    let (exec_sel_tx, _exec_sel_rx) = watch::channel(None);
    let (log_mode_tx, _log_mode_rx) = watch::channel(LogMode::Normalized);
    let (diff_stats_tx, _diff_stats_rx) = watch::channel(false);
    let (diff_reconnect_tx, _diff_reconnect_rx) = watch::channel(0u64);
    let (reconnect_tx, _reconnect_rx) = watch::channel(0u64);

    AppState::new(
        "http://127.0.0.1:1234".to_string(),
        net_tx,
        project_sel_tx,
        attempt_sel_tx,
        exec_sel_tx,
        log_mode_tx,
        diff_stats_tx,
        diff_reconnect_tx,
        reconnect_tx,
        crate::state::TuiPrefs::default(),
    )
}

pub(crate) fn patch_from_json(value: serde_json::Value) -> json_patch::Patch {
    serde_json::from_value(value).expect("valid json-patch array")
}

pub(crate) fn add_project(app: &mut AppState, project_id: Uuid, name: &str) {
    let patch = patch_from_json(serde_json::json!([
        {
            "op": "add",
            "path": format!("/projects/{project_id}"),
            "value": { "name": name }
        }
    ]));
    app.board.projects_store.apply_patch(&patch).unwrap();
    app.board.selected_project_id = Some(project_id);
}

pub(crate) fn add_task(
    app: &mut AppState,
    task_id: Uuid,
    status: TaskStatus,
    title: &str,
    parent_task_id: Option<Uuid>,
    project_id: Option<Uuid>,
) {
    let mut value = serde_json::json!({
        "status": status.as_api_str(),
        "title": title,
    });
    if let Some(parent) = parent_task_id {
        value.as_object_mut().unwrap().insert(
            "parent_task_id".to_string(),
            serde_json::Value::String(parent.to_string()),
        );
    }
    if let Some(project) = project_id {
        value.as_object_mut().unwrap().insert(
            "project_id".to_string(),
            serde_json::Value::String(project.to_string()),
        );
    }
    let patch = patch_from_json(serde_json::json!([
        {
            "op": "add",
            "path": format!("/tasks/{task_id}"),
            "value": value
        }
    ]));
    app.board.tasks_store.apply_patch(&patch).unwrap();
    app.board.selected_task_id = Some(task_id);
    app.board.tasks_active_column = status;
}

pub(crate) fn set_attempts(app: &mut AppState, attempts: Vec<AttemptRow>, selected: Option<Uuid>) {
    app.board.attempts = attempts;
    app.board.selected_attempt_id = selected;
    if let Some(id) = selected {
        if let Some(idx) = app.board.attempts.iter().position(|a| a.id == id) {
            app.board.selected_attempt_index = idx;
        }
    } else {
        app.board.selected_attempt_index = 0;
    }
}

pub(crate) fn diff_root_with_single_file(key: &str, additions: i64, deletions: i64) -> DiffRoot {
    let mut root = DiffRoot::empty();
    let patch = patch_from_json(serde_json::json!([
        {
            "op": "add",
            "path": format!("/entries/{key}"),
            "value": {
                "type": "DIFF",
                "content": { "change": "modified", "additions": additions, "deletions": deletions }
            }
        }
    ]));
    root.apply_patch(&patch).unwrap();
    root
}

pub(crate) fn empty_projects_root() -> ProjectsRoot {
    ProjectsRoot::empty()
}

pub(crate) fn empty_tasks_root() -> TasksRoot {
    TasksRoot::empty()
}
