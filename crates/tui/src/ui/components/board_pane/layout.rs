use crate::{
    selection::BoardTaskItem,
    state::{AppState, TaskStatus},
    store::tasks_board::board_tasks_by_status,
    util::board_statuses,
};

pub(super) fn desired_board_section_height(list_len: usize) -> u16 {
    let inner = (list_len.max(1)).min(u16::MAX as usize) as u16;
    inner.saturating_add(2).max(3)
}

pub(super) fn allocate_board_section_heights(needs: &[u16], available: u16) -> Vec<u16> {
    if needs.is_empty() || available == 0 {
        return vec![];
    }

    // 3 lines is the minimum to show a bordered block + 1 line of content.
    let min_h = 3u16;
    let n = needs.len();

    // If the terminal is absurdly small, just split whatever is available.
    if available < (n as u16).saturating_mul(min_h) {
        let base = (available / n as u16).max(1);
        let mut heights = vec![base; n];
        let mut remaining = available.saturating_sub(base.saturating_mul(n as u16));
        for h in heights.iter_mut() {
            if remaining == 0 {
                break;
            }
            *h = h.saturating_add(1);
            remaining -= 1;
        }
        return heights;
    }

    let needs: Vec<u16> = needs.iter().copied().map(|h| h.max(min_h)).collect();
    let total_need: u16 = needs.iter().copied().sum();
    if total_need <= available {
        return needs;
    }

    let mut heights = vec![min_h; n];
    let mut remaining = available.saturating_sub(min_h.saturating_mul(n as u16));
    let mut deficits: Vec<u16> = needs.iter().map(|h| h.saturating_sub(min_h)).collect();

    while remaining > 0 {
        let mut progressed = false;
        for i in 0..n {
            if remaining == 0 {
                break;
            }
            if deficits[i] > 0 {
                heights[i] = heights[i].saturating_add(1);
                deficits[i] -= 1;
                remaining -= 1;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }

    heights
}

pub(super) fn task_index_in(list: &[BoardTaskItem], task_id: Option<uuid::Uuid>) -> Option<usize> {
    let id = task_id?;
    list.iter().position(|t| t.task.id == id)
}

pub(super) fn board_section_needs(app: &AppState) -> Vec<u16> {
    let by_status = board_tasks_by_status(&app.board.tasks_store, &app.board.task_filter);
    let statuses = board_statuses(app);
    statuses
        .iter()
        .map(|status| {
            let list_len = match status {
                TaskStatus::Todo => by_status.todo.len(),
                TaskStatus::InProgress => by_status.inprogress.len(),
                TaskStatus::InReview => by_status.inreview.len(),
                TaskStatus::Done => by_status.done.len(),
                TaskStatus::Cancelled => by_status.cancelled.len(),
            };
            desired_board_section_height(list_len)
        })
        .collect()
}
