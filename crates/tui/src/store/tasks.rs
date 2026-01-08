use uuid::Uuid;

use crate::state::{TaskRow, TaskStatus, Timestamp};

pub(crate) struct TasksStore<'a> {
    root: &'a serde_json::Value,
}

impl<'a> TasksStore<'a> {
    pub(crate) fn new(root: &'a serde_json::Value) -> Self {
        Self { root }
    }

    pub(crate) fn tasks_object(&self) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
        self.root.get("tasks")?.as_object()
    }

    pub(crate) fn task(&self, task_id: Uuid) -> Option<TaskRow> {
        let task_val = self.tasks_object()?.get(&task_id.to_string())?;

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
            .and_then(Timestamp::parse);
        let parent_task_id = task_val
            .get("parent_task_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
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
            parent_task_id,
            updated_at,
            has_in_progress_attempt,
            last_attempt_failed,
            executor,
            description,
        })
    }

    pub(crate) fn tasks(&self) -> Vec<TaskRow> {
        let Some(tasks_obj) = self.tasks_object() else {
            return vec![];
        };

        let mut rows = Vec::with_capacity(tasks_obj.len());
        for (id_str, _task_val) in tasks_obj.iter() {
            let Ok(id) = Uuid::parse_str(id_str) else {
                continue;
            };
            let Some(task) = self.task(id) else {
                continue;
            };
            rows.push(task);
        }
        rows
    }
}
