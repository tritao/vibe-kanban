use std::collections::HashMap;

use uuid::Uuid;

use crate::state::{TaskRow, TaskStatus};

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

pub(crate) fn board_tasks_by_root_status(tasks: Vec<TaskRow>) -> BoardTasksByStatus {
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
