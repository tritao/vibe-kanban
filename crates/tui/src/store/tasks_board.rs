use crate::store::{tasks_hierarchy, tasks_hierarchy::BoardTasksByStatus, tasks_list};

pub(crate) fn board_tasks_by_status(
    tasks_store: &serde_json::Value,
    task_filter: &str,
) -> BoardTasksByStatus {
    let tasks = tasks_list::tasks_filtered(tasks_store, task_filter);
    tasks_hierarchy::board_tasks_by_root_status(tasks)
}
