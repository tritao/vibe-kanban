use std::time::Duration;

use uuid::Uuid;

use crate::logs::reset_logs;
use crate::net;
use crate::selection::lists_filters::{filtered_projects_for_selection, tasks_filtered_by_status};
use crate::selection::{active_exec_id, exec_list, find_task};
use crate::prefs::save_prefs;
use crate::commands::request_branch_status_refresh;
use crate::state::{AppState, TaskStatus};

pub(crate) fn set_selected_project(app: &mut AppState, project_id: Option<Uuid>) {
    if app.board.selected_project_id == project_id {
        return;
    }
    app.board.selected_project_id = project_id;
    let _ = app.project_sel_tx.send(project_id);
    app.prefs.selected_project_id = project_id;
    save_prefs(&app.prefs);
    // Project switch invalidates task/attempt/exec/log selections immediately.
    set_selected_task(app, None);
}

pub(crate) fn set_selected_task(app: &mut AppState, task_id: Option<Uuid>) {
    if app.board.selected_task_id == task_id {
        return;
    }

    app.board.selected_task_id = task_id;

    // Clear dependent panes.
    app.board.attempts.clear();
    app.board.selected_attempt_index = 0;
    set_selected_attempt(app, None);

    // Fetch attempts for the new task selection.
    if let Some(task_id) = task_id {
        let base_url = app.backend_url.clone();
        let net_tx = app.net_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            net::load_attempts_task(base_url, task_id, net_tx).await;
        });
    }
}

pub(crate) fn set_selected_attempt(app: &mut AppState, attempt_id: Option<Uuid>) {
    if app.board.selected_attempt_id == attempt_id {
        return;
    }

    app.board.selected_attempt_id = attempt_id;

    // Clear transient notices/errors when context changes.
    app.ui.last_error = None;
    app.ui.last_notice = None;

    app.exec.exec_store = serde_json::json!({ "execution_processes": {} });
    set_selected_exec(app, None);
    reset_logs(app, None);

    app.diff.diff_store = serde_json::json!({ "entries": {} });
    app.diff.selected_diff_index = 0;
    app.diff.diff_scroll_offset = 0;

    app.diff.repo_statuses.clear();
    app.diff.selected_repo_index = 0;

    let _ = app.attempt_sel_tx.send(attempt_id);
    request_branch_status_refresh(app);
}

pub(crate) fn set_selected_exec(app: &mut AppState, exec_id: Option<Uuid>) {
    if app.exec.selected_exec_id == exec_id {
        return;
    }

    app.exec.selected_exec_id = exec_id;
    // Clear transient notices/errors when context changes.
    app.ui.last_error = None;
    app.ui.last_notice = None;
    if let Some(exec_id) = exec_id {
        app.exec.log_buffers.entry(exec_id).or_default().ensure_init();
        app.exec.log_view_dirty = true;
    }
    app.exec.log_selected = None;
    let _ = app.exec_sel_tx.send(exec_id);
}

pub(crate) fn ensure_selection_visible(app: &mut AppState) {
    ensure_project_selection(app);
    ensure_task_selection(app);
    ensure_attempt_selection(app);
    ensure_exec_selection(app);
}

pub(crate) fn ensure_project_selection(app: &mut AppState) {
    let projects = filtered_projects_for_selection(app);
    if projects.is_empty() {
        app.board.selected_project_index = 0;
        set_selected_project(app, None);
        return;
    }

    if let Some(id) = app.board.selected_project_id
        && let Some(idx) = projects.iter().position(|p| p.0 == id)
    {
        app.board.selected_project_index = idx;
        return;
    }

    app.board.selected_project_index = 0;
    set_selected_project(app, Some(projects[0].0));
}

pub(crate) fn ensure_task_selection(app: &mut AppState) {
    if app.board.selected_project_id.is_none() {
        set_selected_task(app, None);
        return;
    }

    let list = tasks_filtered_by_status(app, app.board.tasks_active_column);
    if list.is_empty() {
        set_selected_task(app, None);
        return;
    }

    if let Some(id) = app.board.selected_task_id
        && let Some(idx) = list.iter().position(|t| t.id == id)
    {
        app.board.board_index_by_status[app.board.tasks_active_column.idx()] = idx;
        return;
    }

    let idx = app.board.board_index_by_status[app.board.tasks_active_column.idx()].min(list.len() - 1);
    set_selected_task(app, Some(list[idx].id));
}

pub(crate) fn ensure_attempt_selection(app: &mut AppState) {
    if app.board.attempts.is_empty() {
        app.board.selected_attempt_index = 0;
        set_selected_attempt(app, None);
        return;
    }

    if let Some(id) = app.board.selected_attempt_id
        && let Some(idx) = app.board.attempts.iter().position(|a| a.id == id)
    {
        app.board.selected_attempt_index = idx;
        return;
    }

    app.board.selected_attempt_index = app
        .board
        .selected_attempt_index
        .min(app.board.attempts.len() - 1);
    set_selected_attempt(
        app,
        Some(app.board.attempts[app.board.selected_attempt_index].id),
    );
}

pub(crate) fn ensure_exec_selection(app: &mut AppState) {
    let execs = exec_list(&app.exec.exec_store);
    set_selected_exec(app, active_exec_id(&execs));
}

pub(crate) fn sync_tasks_active_column(app: &mut AppState) {
    let Some(task_id) = app.board.selected_task_id else {
        return;
    };
    let Some(task) = find_task(&app.board.tasks_store, task_id) else {
        return;
    };
    app.board.tasks_active_column = match task.status {
        TaskStatus::Cancelled if !app.board.show_cancelled => TaskStatus::Done,
        other => other,
    };
}
