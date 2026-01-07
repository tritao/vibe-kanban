use uuid::Uuid;

use super::ids::{select_attempt, select_exec, select_project, select_task};
use crate::{
    selection::{
        active_exec_id, board_tasks_by_status, exec_list, filtered_projects, find_task,
        tasks_filtered_base,
    },
    state::{AppState, AttemptRow, TaskStatus},
};

pub(in crate::actions) fn reconcile_projects_selection(app: &mut AppState) {
    let projects: Vec<(Uuid, String)> = filtered_projects(app)
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    if projects.is_empty() {
        app.board.selected_project_index = 0;
        select_project(app, None);
        return;
    }

    if let Some(selected_id) = app.board.selected_project_id
        && let Some(idx) = projects.iter().position(|p| p.0 == selected_id)
    {
        app.board.selected_project_index = idx;
        return;
    }

    app.board.selected_project_index = app.board.selected_project_index.min(projects.len() - 1);
    select_project(app, Some(projects[app.board.selected_project_index].0));
}

pub(in crate::actions) fn reconcile_tasks_selection(app: &mut AppState) {
    let tasks = tasks_filtered_base(app);
    if tasks.is_empty() {
        select_task(app, None);
        return;
    }

    if let Some(pending) = app.board.pending_select_task_id {
        if tasks.iter().any(|t| t.id == pending) {
            app.board.pending_select_task_id = None;
            select_task(app, Some(pending));
            sync_tasks_active_column(app);
            ensure_selection_visible(app);
            return;
        }
    }

    if let Some(selected_id) = app.board.selected_task_id {
        if tasks.iter().any(|t| t.id == selected_id) {
            sync_tasks_active_column(app);
            return;
        }
    }

    let by_status = board_tasks_by_status(app);
    let chosen = match app.board.tasks_active_column {
        TaskStatus::Todo => by_status.todo.first().map(|i| &i.task),
        TaskStatus::InProgress => by_status.inprogress.first().map(|i| &i.task),
        TaskStatus::InReview => by_status.inreview.first().map(|i| &i.task),
        TaskStatus::Done => by_status.done.first().map(|i| &i.task),
        TaskStatus::Cancelled => by_status.cancelled.first().map(|i| &i.task),
    }
    .or_else(|| tasks.first());

    select_task(app, chosen.map(|t| t.id));
    sync_tasks_active_column(app);
    ensure_selection_visible(app);
}

pub(in crate::actions) fn set_attempts(app: &mut AppState, attempts: Vec<AttemptRow>) {
    app.board.attempts = attempts;
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

pub(crate) fn ensure_selection_visible(app: &mut AppState) {
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

    let by_status = board_tasks_by_status(app);
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

    if let Some(id) = app.board.selected_task_id
        && let Some(idx) = list.iter().position(|t| t.task.id == id)
    {
        app.board.board_index_by_status[app.board.tasks_active_column.idx()] = idx;
        return;
    }

    let idx =
        app.board.board_index_by_status[app.board.tasks_active_column.idx()].min(list.len() - 1);
    select_task(app, Some(list[idx].task.id));
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

pub(in crate::actions) fn ensure_exec_selection(app: &mut AppState) {
    let execs = exec_list(&app.exec.exec_store);
    select_exec(app, active_exec_id(&execs));
}

pub(in crate::actions) fn sync_tasks_active_column(app: &mut AppState) {
    let Some(task_id) = app.board.selected_task_id else {
        return;
    };
    let by_status = board_tasks_by_status(app);
    for status in crate::util::board_statuses(app) {
        let list = match status {
            TaskStatus::Todo => &by_status.todo,
            TaskStatus::InProgress => &by_status.inprogress,
            TaskStatus::InReview => &by_status.inreview,
            TaskStatus::Done => &by_status.done,
            TaskStatus::Cancelled => &by_status.cancelled,
        };
        if list.iter().any(|i| i.task.id == task_id) {
            app.board.tasks_active_column = status;
            return;
        }
    }

    // Fallback: if the task isn't visible in the current board view, keep prior behavior.
    if let Some(task) = find_task(&app.board.tasks_store, task_id) {
        app.board.tasks_active_column = match task.status {
            TaskStatus::Cancelled if !app.board.show_cancelled => TaskStatus::Done,
            other => other,
        };
    }
}
