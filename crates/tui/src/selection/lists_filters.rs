use uuid::Uuid;

use crate::{
    state::{AppState, ExecRow, ExecStatus, RunReason, TaskRow},
    store::{
        exec::ExecStore, projects::ProjectsStore, tasks::TasksStore, tasks_hierarchy,
        tasks_hierarchy::BoardTasksByStatus,
    },
};

fn contains_ci(haystack: &str, needle: &str) -> bool {
    let needle = needle.trim();
    if needle.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectRow {
    pub(crate) id: Uuid,
    pub(crate) name: String,
}

pub(crate) fn filtered_projects(app: &AppState) -> Vec<ProjectRow> {
    let mut list = projects_list(&app.board.projects_store);
    let q = app.board.project_filter.trim();
    if !q.is_empty() {
        list.retain(|p| contains_ci(&p.name, q));
    }
    list
}

pub(crate) fn projects_list(store: &serde_json::Value) -> Vec<ProjectRow> {
    ProjectsStore::new(store)
        .projects()
        .into_iter()
        .map(|(id, name)| ProjectRow { id, name })
        .collect()
}

pub(crate) fn exec_list(store: &serde_json::Value) -> Vec<ExecRow> {
    ExecStore::new(store).execs()
}

pub(crate) fn active_exec_id(execs: &[ExecRow]) -> Option<Uuid> {
    let mut filtered: Vec<&ExecRow> = execs.iter().filter(|e| !e.dropped).collect();
    if filtered.is_empty() {
        return None;
    }
    // exec_list is sorted oldest -> newest, keep that invariant for selection.
    filtered.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let running: Vec<&ExecRow> = filtered
        .iter()
        .copied()
        .filter(|e| e.status == Some(ExecStatus::Running))
        .collect();
    if !running.is_empty() {
        if let Some(non_dev) = running
            .iter()
            .copied()
            .find(|e| e.run_reason != Some(RunReason::DevServer))
        {
            return Some(non_dev.id);
        }
        return Some(running[running.len() - 1].id);
    }

    if let Some(agent) = filtered
        .iter()
        .rev()
        .copied()
        .find(|e| e.run_reason == Some(RunReason::CodingAgent))
    {
        return Some(agent.id);
    }

    Some(filtered[filtered.len() - 1].id)
}

pub(crate) fn board_tasks_by_status(app: &AppState) -> BoardTasksByStatus {
    tasks_hierarchy::board_tasks_by_root_status(tasks_filtered_base(app))
}

fn tasks_list(store: &serde_json::Value) -> Vec<TaskRow> {
    TasksStore::new(store).tasks()
}

pub(crate) fn tasks_filtered_base(app: &AppState) -> Vec<TaskRow> {
    let mut tasks = tasks_all(&app.board.tasks_store);
    let q = app.board.task_filter.trim();
    if !q.is_empty() {
        tasks.retain(|t| contains_ci(&t.title, q));
    }
    tasks
}

pub(crate) fn tasks_all(store: &serde_json::Value) -> Vec<TaskRow> {
    tasks_list(store)
}

pub(crate) fn find_task(store: &serde_json::Value, task_id: Uuid) -> Option<TaskRow> {
    TasksStore::new(store).task(task_id)
}
