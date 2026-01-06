use std::time::Duration;

use uuid::Uuid;

use super::diff::reset_diff_stream_state;
use crate::{net, prefs::save_prefs, state::AppState};

pub(in crate::actions) fn select_project(app: &mut AppState, project_id: Option<Uuid>) {
    if app.board.selected_project_id == project_id {
        return;
    }
    app.board.selected_project_id = project_id;
    let _ = app.project_sel_tx.send(project_id);
    app.prefs.selected_project_id = project_id;
    save_prefs(&app.prefs);
    select_task(app, None);
}

pub(in crate::actions) fn select_task(app: &mut AppState, task_id: Option<Uuid>) {
    if app.board.selected_task_id == task_id {
        return;
    }

    app.board.selected_task_id = task_id;

    app.board.attempts.clear();
    app.board.selected_attempt_index = 0;
    select_attempt(app, None);

    if let Some(task_id) = task_id {
        let base_url = app.backend_url.clone();
        let net_tx = app.net_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            net::load_attempts_task(base_url, task_id, net_tx).await;
        });
    }
}

pub(in crate::actions) fn select_attempt(app: &mut AppState, attempt_id: Option<Uuid>) {
    if app.board.selected_attempt_id == attempt_id && attempt_id.is_some() {
        return;
    }

    app.board.selected_attempt_id = attempt_id;
    if let Some(attempt_id) = attempt_id {
        if let Some(idx) = app.board.attempts.iter().position(|a| a.id == attempt_id) {
            app.board.selected_attempt_index = idx;
        }
    }

    app.ui.last_error = None;
    app.ui.last_notice = None;

    app.exec.exec_store = serde_json::json!({ "execution_processes": {} });
    select_exec(app, None);
    crate::logs::reset_log_view(app, None);

    reset_diff_stream_state(app);

    app.diff.repo_statuses.clear();
    app.diff.selected_repo_index = 0;

    let _ = app.attempt_sel_tx.send(attempt_id);
    crate::commands::request_branch_status_refresh(app);
}

pub(in crate::actions) fn select_exec(app: &mut AppState, exec_id: Option<Uuid>) {
    if app.exec.selected_exec_id == exec_id {
        return;
    }

    app.exec.selected_exec_id = exec_id;
    app.ui.last_error = None;
    app.ui.last_notice = None;
    if let Some(exec_id) = exec_id {
        app.exec
            .log_buffers
            .entry(exec_id)
            .or_default()
            .ensure_init();
        app.exec.log_view_dirty = true;
    }
    app.exec.log_selected = None;
    let _ = app.exec_sel_tx.send(exec_id);
}
