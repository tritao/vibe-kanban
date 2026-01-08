use uuid::Uuid;

use crate::state::TaskRow;

fn contains_ci(haystack: &str, needle: &str) -> bool {
    let needle = needle.trim();
    if needle.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

pub(crate) fn tasks_all(root: &serde_json::Value) -> Vec<TaskRow> {
    super::tasks::TasksStore::new(root).tasks()
}

pub(crate) fn tasks_filtered(root: &serde_json::Value, task_filter: &str) -> Vec<TaskRow> {
    let mut tasks = tasks_all(root);
    let q = task_filter.trim();
    if !q.is_empty() {
        tasks.retain(|t| contains_ci(&t.title, q));
    }
    tasks
}

pub(crate) fn find_task(root: &serde_json::Value, task_id: Uuid) -> Option<TaskRow> {
    super::tasks::TasksStore::new(root).task(task_id)
}
