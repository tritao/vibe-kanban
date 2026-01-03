use crate::state::{AppState, TaskStatus};

pub(crate) fn board_statuses(app: &AppState) -> Vec<TaskStatus> {
    if app.board.show_cancelled {
        vec![
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::InReview,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ]
    } else {
        vec![
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::InReview,
            TaskStatus::Done,
        ]
    }
}

pub(crate) fn window_for_list(len: usize, selected: usize, height: usize) -> (usize, usize, usize) {
    if len == 0 || height == 0 {
        return (0, 0, 0);
    }

    if len <= height {
        return (0, len, selected.min(len - 1));
    }

    let selected = selected.min(len - 1);
    let mut start = selected.saturating_sub(height / 2);
    start = start.min(len.saturating_sub(height));
    let end = (start + height).min(len);
    (start, end, selected.saturating_sub(start))
}
