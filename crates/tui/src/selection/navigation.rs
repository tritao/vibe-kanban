use std::time::Duration;

use uuid::Uuid;

use crate::diff::diff_rows_with_all;
use crate::diff_preview::schedule_diff_preview_refresh;
use crate::ui;
use crate::util::board_statuses;
use crate::events::NetEvent;
use crate::state::{AppState, TaskRow, TaskStatus};
use crate::net::ops::update_task_status_http;

use super::lists_filters::{tasks_by_status, tasks_filtered_base};
use super::state::{set_selected_attempt, set_selected_task};

pub(crate) fn task_index_in(list: &[TaskRow], selected_id: Option<Uuid>) -> Option<usize> {
    let selected_id = selected_id?;
    list.iter().position(|t| t.id == selected_id)
}

pub(crate) fn select_adjacent_task(app: &mut AppState, delta: i32) {
    let tasks = tasks_filtered_base(app);
    if tasks.is_empty() {
        set_selected_task(app, None);
        return;
    }

    let by_status = tasks_by_status(&tasks);
    let statuses = board_statuses(app);
    if statuses.is_empty() {
        set_selected_task(app, None);
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

    // Determine current (status, index) from the selected task if possible; otherwise fall back to
    // the active column + stored index.
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

    // If the current column is empty, jump to the nearest non-empty column in the movement
    // direction (or the first non-empty).
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
            set_selected_task(app, None);
            return;
        }
    }

    let cur_status_pos = statuses.iter().position(|s| *s == cur_status).unwrap_or(0);
    let cur_list = list_for(cur_status);

    let (next_status, next_idx) = if delta < 0 {
        if cur_idx > 0 {
            (cur_status, cur_idx - 1)
        } else {
            // Move to the last task of the previous non-empty section (if any).
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
    } else {
        if cur_idx + 1 < cur_list.len() {
            (cur_status, cur_idx + 1)
        } else {
            // Move to the first task of the next non-empty section (if any).
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
        }
    };

    let next_list = list_for(next_status);
    if next_list.is_empty() {
        set_selected_task(app, None);
        return;
    }
    let next_idx = next_idx.min(next_list.len() - 1);
    app.board.tasks_active_column = next_status;
    app.board.board_index_by_status[next_status.idx()] = next_idx;
    set_selected_task(app, Some(next_list[next_idx].id));
}

pub(crate) fn clamp_index(cur: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta < 0 {
        cur.saturating_sub(delta.unsigned_abs() as usize)
    } else {
        (cur + delta as usize).min(len - 1)
    }
}

pub(crate) fn request_move_selected_task(app: &mut AppState, direction: i32) {
    let Some(task_id) = app.board.selected_task_id else {
        return;
    };
    let Some(task) = super::lists_filters::find_task(&app.board.tasks_store, task_id) else {
        return;
    };
    let Some(next) = next_status(task.status, direction) else {
        return;
    };

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = update_task_status_http(&base_url, task_id, next).await {
            let _ = net_tx
                .send(NetEvent::Error(format!("status update failed: {e}")))
                .await;
        }
    });
}

pub(crate) fn ensure_selected_task_in_active_column(app: &mut AppState) {
    let tasks = tasks_filtered_base(app);
    if tasks.is_empty() {
        set_selected_task(app, None);
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
        set_selected_task(app, None);
        return;
    }

    if let Some(selected_id) = app.board.selected_task_id
        && let Some(idx) = list.iter().position(|t| t.id == selected_id)
    {
        app.board.board_index_by_status[app.board.tasks_active_column.idx()] = idx;
        return;
    }

    let idx = app.board.board_index_by_status[app.board.tasks_active_column.idx()].min(list.len() - 1);
    set_selected_task(app, Some(list[idx].id));
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

pub(crate) fn select_adjacent_attempt(app: &mut AppState, delta: i32) {
    if app.board.attempts.is_empty() {
        return;
    }

    let cur = app
        .board
        .selected_attempt_id
        .and_then(|id| app.board.attempts.iter().position(|a| a.id == id))
        .unwrap_or(app.board.selected_attempt_index.min(app.board.attempts.len() - 1));

    let next = clamp_index(cur, delta, app.board.attempts.len());
    if next == cur {
        return;
    }

    app.board.selected_attempt_index = next;
    let id = app.board.attempts.get(next).map(|a| a.id);
    set_selected_attempt(app, id);
}

pub(crate) fn move_active_status(app: &mut AppState, delta: i32) {
    let statuses = board_statuses(app);
    if statuses.is_empty() {
        return;
    }

    let cur = statuses
        .iter()
        .position(|s| *s == app.board.tasks_active_column)
        .unwrap_or(0);
    let next = clamp_index(cur, delta, statuses.len());
    app.board.tasks_active_column = statuses[next];
    ensure_selected_task_in_active_column(app);
}

pub(crate) fn select_adjacent_diff_file(app: &mut AppState, delta: i32) {
    let rows = diff_rows_with_all(&app.diff.diff_store);
    if rows.is_empty() {
        app.diff.selected_diff_index = 0;
        return;
    }

    let cur = app.diff.selected_diff_index.min(rows.len() - 1);
    let next = clamp_index(cur, delta, rows.len());
    if next == cur {
        return;
    }

    app.diff.selected_diff_index = next;
    app.diff.diff_scroll_offset = 0;
    ui::sync_selected_repo_from_diff_selection(app);
    schedule_diff_preview_refresh(app, Duration::from_millis(0));
}
