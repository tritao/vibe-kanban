use uuid::Uuid;

use super::ids::{select_attempt, select_task};
use crate::{
    events::NetOpError,
    state::{AppState, TaskStatus},
    store::{tasks_board::board_tasks_by_status, tasks_list::find_task},
    ui::components::BoardHit,
};
pub(in crate::actions) fn ensure_selected_task_in_active_column(app: &mut AppState) {
    let by_status = board_tasks_by_status(app.board.tasks_store.as_value(), &app.board.task_filter);
    let list = match app.board.tasks_active_column {
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
        && let Some(idx) = list.iter().position(|t| t.task.id == selected_id)
    {
        app.board.board_index_by_status[app.board.tasks_active_column.idx()] = idx;
        return;
    }

    let idx =
        app.board.board_index_by_status[app.board.tasks_active_column.idx()].min(list.len() - 1);
    select_task(app, Some(list[idx].task.id));
}

pub(in crate::actions) fn select_adjacent_attempt(app: &mut AppState, delta: i32) {
    if app.board.attempts.is_empty() {
        return;
    }

    if let Some(id) = app.board.selected_attempt_id
        && let Some(idx) = app.board.attempts.iter().position(|a| a.id == id)
    {
        app.board.selected_attempt_index = idx;
    }

    let mut idx = app
        .board
        .selected_attempt_index
        .min(app.board.attempts.len() - 1);
    if !crate::ui::list_nav::select_delta(&mut idx, delta, app.board.attempts.len()) {
        return;
    }
    app.board.selected_attempt_index = idx;
    select_attempt(app, app.board.attempts.get(idx).map(|a| a.id));
}

pub(crate) fn move_active_status(app: &mut AppState, delta: i32) {
    let statuses = crate::util::board_statuses(app);
    if statuses.is_empty() {
        return;
    }

    let cur = statuses
        .iter()
        .position(|s| *s == app.board.tasks_active_column)
        .unwrap_or(0);
    let mut idx = cur;
    if !crate::ui::list_nav::select_delta(&mut idx, delta, statuses.len()) {
        return;
    }
    app.board.tasks_active_column = statuses[idx];
    ensure_selected_task_in_active_column(app);
}

pub(crate) fn select_adjacent_task(app: &mut AppState, delta: i32) {
    let by_status = board_tasks_by_status(app.board.tasks_store.as_value(), &app.board.task_filter);
    if by_status.todo.is_empty()
        && by_status.inprogress.is_empty()
        && by_status.inreview.is_empty()
        && by_status.done.is_empty()
        && by_status.cancelled.is_empty()
    {
        select_task(app, None);
        return;
    }

    let statuses = crate::util::board_statuses(app);
    if statuses.is_empty() {
        select_task(app, None);
        return;
    }

    let list_for = |status: TaskStatus| -> &[crate::selection::BoardTaskItem] {
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
            if let Some(pos) = list.iter().position(|t| t.task.id == id) {
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
    select_task(app, Some(next_list[next_idx].task.id));
}

pub(crate) fn focus_board_section(app: &mut AppState, status: TaskStatus) {
    if app.board.tasks_active_column != status {
        app.board.tasks_active_column = status;
    }
    ensure_selected_task_in_active_column(app);
}

pub(crate) fn apply_board_hit(app: &mut AppState, hit: BoardHit) {
    focus_board_section(app, hit.status);

    if let Some(idx) = hit.clicked_index {
        app.board.board_index_by_status[hit.status.idx()] = idx;
    }

    if let Some(task_id) = hit.clicked_task_id {
        select_task(app, Some(task_id));
    } else {
        ensure_selected_task_in_active_column(app);
    }
}

pub(crate) fn normalize_after_cancelled_toggle(app: &mut AppState) {
    if !app.board.show_cancelled && app.board.tasks_active_column == TaskStatus::Cancelled {
        app.board.tasks_active_column = TaskStatus::Done;
    }
    ensure_selected_task_in_active_column(app);
}

pub(in crate::actions) fn note_task_created(app: &mut AppState, task_id: Uuid, status: TaskStatus) {
    app.board.pending_select_task_id = Some(task_id);
    focus_board_section(app, status);
}

pub(crate) fn request_move_selected_task(app: &mut AppState, direction: i32) {
    let Some(task_id) = app.board.selected_task_id else {
        return;
    };
    let Some(task) = find_task(app.board.tasks_store.as_value(), task_id) else {
        return;
    };
    let Some(next) = next_status(task.status, direction) else {
        return;
    };

    crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
        if let Err(e) = crate::net::ops::update_task_status_http(&base_url, task_id, next).await {
            let _ = net_tx
                .send(NetOpError::new("update task status", e).into_event())
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
