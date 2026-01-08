use std::collections::HashMap;

use uuid::Uuid;

use crate::{
    state::{AppState, ExecRow, ExecStatus, RunReason, TaskRow, TaskStatus},
    store::{exec::ExecStore, projects::ProjectsStore, tasks::TasksStore},
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

#[derive(Debug, Clone)]
pub(crate) struct BoardTaskItem {
    pub(crate) task: TaskRow,
    pub(crate) indent: u8,
    pub(crate) root_status: TaskStatus,
}

pub(crate) struct BoardTasksByStatus {
    pub(crate) todo: Vec<BoardTaskItem>,
    pub(crate) inprogress: Vec<BoardTaskItem>,
    pub(crate) inreview: Vec<BoardTaskItem>,
    pub(crate) done: Vec<BoardTaskItem>,
    pub(crate) cancelled: Vec<BoardTaskItem>,
}

fn sort_task_ids(tasks_by_id: &HashMap<Uuid, TaskRow>, ids: &mut [Uuid]) {
    ids.sort_by(|a, b| {
        let Some(ta) = tasks_by_id.get(a) else {
            return std::cmp::Ordering::Equal;
        };
        let Some(tb) = tasks_by_id.get(b) else {
            return std::cmp::Ordering::Equal;
        };
        tb.updated_at
            .cmp(&ta.updated_at)
            .then_with(|| ta.title.cmp(&tb.title))
    });
}

fn push_subtree(
    out: &mut Vec<BoardTaskItem>,
    tasks_by_id: &HashMap<Uuid, TaskRow>,
    children: &HashMap<Option<Uuid>, Vec<Uuid>>,
    root_status: TaskStatus,
    parent: Uuid,
    indent: u8,
) {
    let Some(child_ids) = children.get(&Some(parent)) else {
        return;
    };
    let mut child_ids = child_ids.clone();
    sort_task_ids(tasks_by_id, &mut child_ids);
    for id in child_ids {
        let Some(t) = tasks_by_id.get(&id) else {
            continue;
        };
        out.push(BoardTaskItem {
            task: t.clone(),
            indent,
            root_status,
        });
        push_subtree(
            out,
            tasks_by_id,
            children,
            root_status,
            id,
            indent.saturating_add(1),
        );
    }
}

pub(crate) fn board_tasks_by_status(app: &AppState) -> BoardTasksByStatus {
    let tasks = tasks_filtered_base(app);
    let mut tasks_by_id: HashMap<Uuid, TaskRow> = HashMap::with_capacity(tasks.len());
    for t in tasks {
        tasks_by_id.insert(t.id, t);
    }

    // Treat tasks with missing parents as top-level so they remain visible (e.g. during filtering).
    let mut children_by_parent: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();
    for t in tasks_by_id.values() {
        let parent = t.parent_task_id.filter(|pid| tasks_by_id.contains_key(pid));
        children_by_parent.entry(parent).or_default().push(t.id);
    }

    let mut out = BoardTasksByStatus {
        todo: vec![],
        inprogress: vec![],
        inreview: vec![],
        done: vec![],
        cancelled: vec![],
    };

    let mut roots: Vec<Uuid> = children_by_parent.get(&None).cloned().unwrap_or_default();
    sort_task_ids(&tasks_by_id, &mut roots);

    for id in roots {
        let Some(t) = tasks_by_id.get(&id) else {
            continue;
        };
        let root_status = t.status;
        let slot = match root_status {
            TaskStatus::Todo => &mut out.todo,
            TaskStatus::InProgress => &mut out.inprogress,
            TaskStatus::InReview => &mut out.inreview,
            TaskStatus::Done => &mut out.done,
            TaskStatus::Cancelled => &mut out.cancelled,
        };
        slot.push(BoardTaskItem {
            task: t.clone(),
            indent: 0,
            root_status,
        });
        push_subtree(slot, &tasks_by_id, &children_by_parent, root_status, id, 1);
    }

    out
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
