use uuid::Uuid;

use crate::state::{AppState, ExecRow, TaskRow, TaskStatus};

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
    let projects_obj = store.get("projects").and_then(|v| v.as_object());
    let Some(projects_obj) = projects_obj else {
        return vec![];
    };

    let mut rows = Vec::with_capacity(projects_obj.len());
    for (id_str, project) in projects_obj.iter() {
        let Ok(id) = Uuid::parse_str(id_str) else {
            continue;
        };
        let name = project
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| project.get("title").and_then(|v| v.as_str()))
            .unwrap_or("(unnamed)")
            .to_string();
        rows.push(ProjectRow { id, name });
    }

    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    rows
}

pub(crate) fn exec_list(store: &serde_json::Value) -> Vec<ExecRow> {
    let exec_obj = store.get("execution_processes").and_then(|v| v.as_object());
    let Some(exec_obj) = exec_obj else {
        return vec![];
    };

    let mut rows = Vec::with_capacity(exec_obj.len());
    for (id_str, exec) in exec_obj.iter() {
        let Ok(id) = Uuid::parse_str(id_str) else {
            continue;
        };
        let session_id = exec
            .get("session_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        let run_reason = exec
            .get("run_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let status = exec
            .get("status")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let created_at = exec
            .get("created_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let dropped = exec
            .get("dropped")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        rows.push(ExecRow {
            id,
            session_id,
            run_reason,
            status,
            created_at,
            dropped,
        });
    }

    rows.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    rows
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
        .filter(|e| e.status.as_deref() == Some("running"))
        .collect();
    if !running.is_empty() {
        if let Some(non_dev) = running
            .iter()
            .copied()
            .find(|e| e.run_reason.as_deref() != Some("dev_server"))
        {
            return Some(non_dev.id);
        }
        return Some(running[running.len() - 1].id);
    }

    if let Some(agent) = filtered
        .iter()
        .rev()
        .copied()
        .find(|e| e.run_reason.as_deref() == Some("coding_agent"))
    {
        return Some(agent.id);
    }

    Some(filtered[filtered.len() - 1].id)
}

pub(crate) struct TasksByStatus {
    pub(crate) todo: Vec<TaskRow>,
    pub(crate) inprogress: Vec<TaskRow>,
    pub(crate) inreview: Vec<TaskRow>,
    pub(crate) done: Vec<TaskRow>,
    pub(crate) cancelled: Vec<TaskRow>,
}

pub(crate) fn tasks_by_status(tasks: &[TaskRow]) -> TasksByStatus {
    let mut out = TasksByStatus {
        todo: vec![],
        inprogress: vec![],
        inreview: vec![],
        done: vec![],
        cancelled: vec![],
    };

    for t in tasks {
        match t.status {
            TaskStatus::Todo => out.todo.push(t.clone()),
            TaskStatus::InProgress => out.inprogress.push(t.clone()),
            TaskStatus::InReview => out.inreview.push(t.clone()),
            TaskStatus::Done => out.done.push(t.clone()),
            TaskStatus::Cancelled => out.cancelled.push(t.clone()),
        }
    }

    let sort = |a: &TaskRow, b: &TaskRow| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.title.cmp(&b.title))
    };
    out.todo.sort_by(sort);
    out.inprogress.sort_by(sort);
    out.inreview.sort_by(sort);
    out.done.sort_by(sort);
    out.cancelled.sort_by(sort);

    out
}

fn tasks_list(store: &serde_json::Value) -> Vec<TaskRow> {
    let tasks_obj = store.get("tasks").and_then(|v| v.as_object());
    let Some(tasks_obj) = tasks_obj else {
        return vec![];
    };

    let mut rows = Vec::with_capacity(tasks_obj.len());
    for (id_str, task_val) in tasks_obj.iter() {
        let Ok(id) = Uuid::parse_str(id_str) else {
            continue;
        };
        let status_str = task_val.get("status").and_then(|v| v.as_str());
        let Some(status) = status_str.and_then(TaskStatus::from_str) else {
            continue;
        };
        let title = task_val
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("(untitled)")
            .to_string();
        let updated_at = task_val
            .get("updated_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let has_in_progress_attempt = task_val
            .get("has_in_progress_attempt")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let last_attempt_failed = task_val
            .get("last_attempt_failed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let executor = task_val
            .get("executor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let description = task_val
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        rows.push(TaskRow {
            id,
            title,
            status,
            updated_at,
            has_in_progress_attempt,
            last_attempt_failed,
            executor,
            description,
        });
    }

    rows
}

pub(crate) fn tasks_filtered_base(app: &AppState) -> Vec<TaskRow> {
    let mut tasks = tasks_list(&app.board.tasks_store);
    let q = app.board.task_filter.trim();
    if !q.is_empty() {
        tasks.retain(|t| contains_ci(&t.title, q));
    }
    tasks
}

pub(crate) fn tasks_filtered_by_status(app: &AppState, status: TaskStatus) -> Vec<TaskRow> {
    let mut tasks = tasks_filtered_base(app);
    tasks.retain(|t| t.status == status);
    tasks.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.title.cmp(&b.title))
    });
    tasks
}

pub(crate) fn find_task(store: &serde_json::Value, task_id: Uuid) -> Option<TaskRow> {
    let task_val = store
        .get("tasks")
        .and_then(|v| v.as_object())?
        .get(&task_id.to_string())?;

    let status_str = task_val.get("status").and_then(|v| v.as_str())?;
    let status = TaskStatus::from_str(status_str)?;
    let title = task_val
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(untitled)")
        .to_string();

    let updated_at = task_val
        .get("updated_at")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let has_in_progress_attempt = task_val
        .get("has_in_progress_attempt")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let last_attempt_failed = task_val
        .get("last_attempt_failed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let executor = task_val
        .get("executor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let description = task_val
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(TaskRow {
        id: task_id,
        title,
        status,
        updated_at,
        has_in_progress_attempt,
        last_attempt_failed,
        executor,
        description,
    })
}

pub(super) fn filtered_projects_for_selection(app: &AppState) -> Vec<(Uuid, String)> {
    // Helper for selection logic to avoid exporting ProjectRow.
    filtered_projects(app)
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect()
}
