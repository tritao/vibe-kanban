use std::time::Duration;

use uuid::Uuid;

use crate::diff::diff_rows_with_all;
use crate::diff_preview::schedule_diff_preview_refresh;
use crate::events::NetEvent;
use crate::net;
use crate::prefs::save_prefs;
use crate::selection::lists_filters::tasks_filtered_by_status;
use crate::selection::{
    active_exec_id, exec_list, filtered_projects, find_task, tasks_by_status, tasks_filtered_base,
};
use crate::state::{AppState, TaskRow, TaskStatus};
use crate::ui::sync_selected_repo_from_diff_selection;

pub(super) fn select_project(app: &mut AppState, project_id: Option<Uuid>) {
    if app.board.selected_project_id == project_id {
        return;
    }
    app.board.selected_project_id = project_id;
    let _ = app.project_sel_tx.send(project_id);
    app.prefs.selected_project_id = project_id;
    save_prefs(&app.prefs);
    select_task(app, None);
}

pub(super) fn select_task(app: &mut AppState, task_id: Option<Uuid>) {
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

pub(super) fn select_attempt(app: &mut AppState, attempt_id: Option<Uuid>) {
    if app.board.selected_attempt_id == attempt_id {
        return;
    }

    app.board.selected_attempt_id = attempt_id;

    app.ui.last_error = None;
    app.ui.last_notice = None;

    app.exec.exec_store = serde_json::json!({ "execution_processes": {} });
    select_exec(app, None);
    crate::logs::reset_logs(app, None);

    app.diff.diff_store = serde_json::json!({ "entries": {} });
    app.diff.selected_diff_index = 0;
    app.diff.diff_scroll_offset = 0;
    app.diff.diff_preview_cache_key = None;
    app.diff.diff_preview_cache_hash = 0;
    app.diff.diff_preview_pending = false;
    app.diff.diff_preview_next_refresh_at = None;
    crate::diff_preview::cancel_diff_preview_job(app);

    app.diff.repo_statuses.clear();
    app.diff.selected_repo_index = 0;

    let _ = app.attempt_sel_tx.send(attempt_id);
    crate::commands::request_branch_status_refresh(app);
}

pub(super) fn select_exec(app: &mut AppState, exec_id: Option<Uuid>) {
    if app.exec.selected_exec_id == exec_id {
        return;
    }

    app.exec.selected_exec_id = exec_id;
    app.ui.last_error = None;
    app.ui.last_notice = None;
    if let Some(exec_id) = exec_id {
        app.exec.log_buffers.entry(exec_id).or_default().ensure_init();
        app.exec.log_view_dirty = true;
    }
    app.exec.log_selected = None;
    let _ = app.exec_sel_tx.send(exec_id);
}

pub(super) fn ensure_selection_visible(app: &mut AppState) {
    ensure_project_selection(app);
    ensure_task_selection(app);
    ensure_attempt_selection(app);
    ensure_exec_selection(app);
}

fn ensure_project_selection(app: &mut AppState) {
    let projects: Vec<(Uuid, String)> = filtered_projects(app)
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    if projects.is_empty() {
        app.board.selected_project_index = 0;
        select_project(app, None);
        return;
    }

    if let Some(id) = app.board.selected_project_id
        && let Some(idx) = projects.iter().position(|p| p.0 == id)
    {
        app.board.selected_project_index = idx;
        return;
    }

    app.board.selected_project_index = 0;
    select_project(app, Some(projects[0].0));
}

fn ensure_task_selection(app: &mut AppState) {
    if app.board.selected_project_id.is_none() {
        select_task(app, None);
        return;
    }

    let list = tasks_filtered_by_status(app, app.board.tasks_active_column);
    if list.is_empty() {
        select_task(app, None);
        return;
    }

    if let Some(id) = app.board.selected_task_id
        && let Some(idx) = list.iter().position(|t| t.id == id)
    {
        app.board.board_index_by_status[app.board.tasks_active_column.idx()] = idx;
        return;
    }

    let idx = app.board.board_index_by_status[app.board.tasks_active_column.idx()].min(list.len() - 1);
    select_task(app, Some(list[idx].id));
}

fn ensure_attempt_selection(app: &mut AppState) {
    if app.board.attempts.is_empty() {
        app.board.selected_attempt_index = 0;
        select_attempt(app, None);
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
    select_attempt(
        app,
        Some(app.board.attempts[app.board.selected_attempt_index].id),
    );
}

pub(super) fn ensure_exec_selection(app: &mut AppState) {
    let execs = exec_list(&app.exec.exec_store);
    select_exec(app, active_exec_id(&execs));
}

pub(super) fn sync_tasks_active_column(app: &mut AppState) {
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

pub(super) fn ensure_selected_task_in_active_column(app: &mut AppState) {
    let tasks = tasks_filtered_base(app);
    if tasks.is_empty() {
        select_task(app, None);
        return;
    }

    let by_status = tasks_by_status(&tasks);
    let list: &[TaskRow] = match app.board.tasks_active_column {
        TaskStatus::Todo => &by_status.todo,
        TaskStatus::InProgress => &by_status.inprogress,
        TaskStatus::InReview => &by_status.inreview,
        TaskStatus::Done => &by_status.done,
        TaskStatus::Cancelled => &by_status.cancelled,
    };
    if list.is_empty() {
        select_task(app, None);
        return;
    }

    if let Some(selected_id) = app.board.selected_task_id
        && let Some(idx) = list.iter().position(|t| t.id == selected_id)
    {
        app.board.board_index_by_status[app.board.tasks_active_column.idx()] = idx;
        return;
    }

    let idx = app.board.board_index_by_status[app.board.tasks_active_column.idx()].min(list.len() - 1);
    select_task(app, Some(list[idx].id));
}

pub(super) fn select_adjacent_attempt(app: &mut AppState, delta: i32) {
    if app.board.attempts.is_empty() {
        return;
    }

    let cur = app
        .board
        .selected_attempt_id
        .and_then(|id| app.board.attempts.iter().position(|a| a.id == id))
        .unwrap_or(app.board.selected_attempt_index.min(app.board.attempts.len() - 1));

    let next = crate::selection::clamp_index(cur, delta, app.board.attempts.len());
    if next == cur {
        return;
    }

    app.board.selected_attempt_index = next;
    let id = app.board.attempts.get(next).map(|a| a.id);
    select_attempt(app, id);
}

pub(super) fn move_active_status(app: &mut AppState, delta: i32) {
    let statuses = crate::util::board_statuses(app);
    if statuses.is_empty() {
        return;
    }

    let cur = statuses
        .iter()
        .position(|s| *s == app.board.tasks_active_column)
        .unwrap_or(0);
    let next = crate::selection::clamp_index(cur, delta, statuses.len());
    app.board.tasks_active_column = statuses[next];
    ensure_selected_task_in_active_column(app);
}

pub(super) fn select_adjacent_task(app: &mut AppState, delta: i32) {
    let tasks = tasks_filtered_base(app);
    if tasks.is_empty() {
        select_task(app, None);
        return;
    }

    let by_status = tasks_by_status(&tasks);
    let statuses = crate::util::board_statuses(app);
    if statuses.is_empty() {
        select_task(app, None);
        return;
    }

    let list_for = |status: TaskStatus| -> &[TaskRow] {
        match status {
            TaskStatus::Todo => &by_status.todo,
            TaskStatus::InProgress => &by_status.inprogress,
            TaskStatus::InReview => &by_status.inreview,
            TaskStatus::Done => &by_status.done,
            TaskStatus::Cancelled => &by_status.cancelled,
        }
    };

    let mut cur_status = app.board.tasks_active_column;
    if !statuses.contains(&cur_status) {
        cur_status = *statuses.last().unwrap_or(&TaskStatus::Done);
    }
    let mut cur_idx = app.board.board_index_by_status[cur_status.idx()];
    if let Some(id) = app.board.selected_task_id {
        for status in &statuses {
            let list = list_for(*status);
            if let Some(pos) = list.iter().position(|t| t.id == id) {
                cur_status = *status;
                cur_idx = pos;
                break;
            }
        }
    }

    if list_for(cur_status).is_empty() {
        let order: Box<dyn Iterator<Item = TaskStatus>> = if delta < 0 {
            Box::new(statuses.iter().copied().rev())
        } else {
            Box::new(statuses.iter().copied())
        };
        if let Some(status) = order.into_iter().find(|s| !list_for(*s).is_empty()) {
            cur_status = status;
            cur_idx = if delta < 0 {
                list_for(cur_status).len().saturating_sub(1)
            } else {
                0
            };
        } else {
            select_task(app, None);
            return;
        }
    }

    let cur_status_pos = statuses.iter().position(|s| *s == cur_status).unwrap_or(0);
    let cur_list = list_for(cur_status);

    let (next_status, next_idx) = if delta < 0 {
        if cur_idx > 0 {
            (cur_status, cur_idx - 1)
        } else {
            let mut s_pos = cur_status_pos;
            let mut found: Option<(TaskStatus, usize)> = None;
            while s_pos > 0 {
                s_pos -= 1;
                let s = statuses[s_pos];
                let list = list_for(s);
                if !list.is_empty() {
                    found = Some((s, list.len() - 1));
                    break;
                }
            }
            found.unwrap_or((cur_status, 0))
        }
    } else if cur_idx + 1 < cur_list.len() {
        (cur_status, cur_idx + 1)
    } else {
        let mut s_pos = cur_status_pos + 1;
        let mut found: Option<(TaskStatus, usize)> = None;
        while s_pos < statuses.len() {
            let s = statuses[s_pos];
            let list = list_for(s);
            if !list.is_empty() {
                found = Some((s, 0));
                break;
            }
            s_pos += 1;
        }
        found.unwrap_or((cur_status, cur_idx))
    };

    let next_list = list_for(next_status);
    if next_list.is_empty() {
        select_task(app, None);
        return;
    }
    let next_idx = next_idx.min(next_list.len() - 1);
    app.board.tasks_active_column = next_status;
    app.board.board_index_by_status[next_status.idx()] = next_idx;
    select_task(app, Some(next_list[next_idx].id));
}

pub(super) fn select_adjacent_diff_file(app: &mut AppState, delta: i32) {
    let rows = diff_rows_with_all(&app.diff.diff_store);
    if rows.is_empty() {
        app.diff.selected_diff_index = 0;
        return;
    }

    let cur = app.diff.selected_diff_index.min(rows.len() - 1);
    let next = crate::selection::clamp_index(cur, delta, rows.len());
    if next == cur {
        return;
    }

    app.diff.selected_diff_index = next;
    app.diff.diff_scroll_offset = 0;
    sync_selected_repo_from_diff_selection(app);
    schedule_diff_preview_refresh(app, Duration::from_millis(0));
}

pub(super) fn select_diff_file(app: &mut AppState, idx: usize) {
    let rows = diff_rows_with_all(&app.diff.diff_store);
    if rows.is_empty() {
        app.diff.selected_diff_index = 0;
        return;
    }

    let next = idx.min(rows.len().saturating_sub(1));
    if next == app.diff.selected_diff_index {
        return;
    }

    app.diff.selected_diff_index = next;
    app.diff.diff_scroll_offset = 0;
    sync_selected_repo_from_diff_selection(app);
    schedule_diff_preview_refresh(app, Duration::from_millis(0));
}

pub(super) fn request_move_selected_task(app: &mut AppState, direction: i32) {
    let Some(task_id) = app.board.selected_task_id else {
        return;
    };
    let Some(task) = find_task(&app.board.tasks_store, task_id) else {
        return;
    };
    let Some(next) = next_status(task.status, direction) else {
        return;
    };

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::net::ops::update_task_status_http(&base_url, task_id, next).await {
            let _ = net_tx
                .send(NetEvent::Error(format!("status update failed: {e}")))
                .await;
        }
    });
}

fn next_status(status: TaskStatus, direction: i32) -> Option<TaskStatus> {
    let chain = [
        TaskStatus::Todo,
        TaskStatus::InProgress,
        TaskStatus::InReview,
        TaskStatus::Done,
    ];
    let idx = chain.iter().position(|s| *s == status)?;
    if direction < 0 {
        idx.checked_sub(1).map(|i| chain[i])
    } else {
        chain.get(idx + 1).copied()
    }
}
