use ratatui::layout::{Constraint, Direction, Layout, Rect};

use super::BoardHit;
use crate::{
    layout::rect_contains,
    selection::{BoardTaskItem, board_tasks_by_status},
    state::{AppState, TaskStatus},
    util::{board_statuses, window_for_list},
};

pub(super) fn board_hit_at(app: &AppState, area: Rect, col: u16, row: u16) -> Option<BoardHit> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let by_status = board_tasks_by_status(app);
    let statuses = board_statuses(app);
    let needs = super::layout::board_section_needs(app);
    let heights = super::layout::allocate_board_section_heights(&needs, area.height);
    let mut constraints: Vec<Constraint> = heights
        .iter()
        .copied()
        .map(|h| Constraint::Length(h))
        .collect();
    constraints.push(Constraint::Min(0));

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (idx, status) in statuses.iter().copied().enumerate() {
        if idx >= sections.len() {
            break;
        }
        let rect = sections[idx];
        if !rect_contains(rect, col, row) {
            continue;
        }

        let list: &[BoardTaskItem] = match status {
            TaskStatus::Todo => &by_status.todo,
            TaskStatus::InProgress => &by_status.inprogress,
            TaskStatus::InReview => &by_status.inreview,
            TaskStatus::Done => &by_status.done,
            TaskStatus::Cancelled => &by_status.cancelled,
        };

        if list.is_empty() {
            return Some(BoardHit {
                status,
                clicked_index: None,
                clicked_task_id: None,
            });
        }

        let inner_y0 = rect.y.saturating_add(1);
        let inner_y1 = rect.y.saturating_add(rect.height).saturating_sub(1);
        if row < inner_y0 || row >= inner_y1 {
            return Some(BoardHit {
                status,
                clicked_index: None,
                clicked_task_id: None,
            });
        }

        let height = rect.height.saturating_sub(2) as usize;
        if height == 0 {
            return Some(BoardHit {
                status,
                clicked_index: None,
                clicked_task_id: None,
            });
        }

        let is_active = status == app.board.tasks_active_column;
        let selected_idx = if is_active {
            super::layout::task_index_in(list, app.board.selected_task_id)
                .or_else(|| {
                    (!list.is_empty()).then_some(
                        app.board.board_index_by_status[status.idx()]
                            .min(list.len().saturating_sub(1)),
                    )
                })
                .unwrap_or(0)
        } else {
            0
        };

        let (start, end, _) = window_for_list(list.len(), selected_idx, height);
        let visible_len = end.saturating_sub(start);
        let inner_row = row.saturating_sub(inner_y0) as usize;
        if inner_row >= visible_len {
            return Some(BoardHit {
                status,
                clicked_index: None,
                clicked_task_id: None,
            });
        }

        let clicked_index = start + inner_row;
        let clicked_task_id = list.get(clicked_index).map(|t| t.task.id);
        return Some(BoardHit {
            status,
            clicked_index: Some(clicked_index),
            clicked_task_id,
        });
    }

    None
}
