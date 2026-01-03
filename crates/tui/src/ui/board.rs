use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
};

use crate::layout::rect_contains;
use crate::render::render_task_line;
use crate::selection::{task_index_in, tasks_by_status, tasks_filtered_base};
use crate::state::{AppState, FocusPane, TaskRow, TaskStatus};
use crate::util::{board_statuses, window_for_list};

#[derive(Debug, Clone, Copy)]
pub(crate) struct BoardHit {
    pub(crate) status: TaskStatus,
    pub(crate) clicked_index: Option<usize>,
    pub(crate) clicked_task_id: Option<uuid::Uuid>,
}

fn desired_board_section_height(list_len: usize) -> u16 {
    let inner = (list_len.max(1)).min(u16::MAX as usize) as u16;
    inner.saturating_add(2).max(3)
}

fn allocate_board_section_heights(needs: &[u16], available: u16) -> Vec<u16> {
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

pub(crate) fn board_hit_at(app: &AppState, area: Rect, col: u16, row: u16) -> Option<BoardHit> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let tasks = tasks_filtered_base(app);
    let by_status = tasks_by_status(&tasks);
    let statuses = board_statuses(app);
    let needs: Vec<u16> = statuses
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
        .collect();
    let heights = allocate_board_section_heights(&needs, area.height);
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

        let list: &[TaskRow] = match status {
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
            task_index_in(list, app.board.selected_task_id)
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
        let clicked_task_id = list.get(clicked_index).map(|t| t.id);
        return Some(BoardHit {
            status,
            clicked_index: Some(clicked_index),
            clicked_task_id,
        });
    }

    None
}

pub(crate) fn render_board_pane(f: &mut Frame, app: &AppState, area: Rect) {
    let tasks = tasks_filtered_base(app);
    let by_status = tasks_by_status(&tasks);
    let statuses = board_statuses(app);
    let needs: Vec<u16> = statuses
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
        .collect();
    let heights = allocate_board_section_heights(&needs, area.height);
    let mut constraints: Vec<Constraint> = heights
        .iter()
        .copied()
        .map(|h| Constraint::Length(h))
        .collect();
    constraints.push(Constraint::Min(0)); // filler

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (idx, status) in statuses.iter().copied().enumerate() {
        if idx >= sections.len() {
            break;
        }

        let list: &[TaskRow] = match status {
            TaskStatus::Todo => &by_status.todo,
            TaskStatus::InProgress => &by_status.inprogress,
            TaskStatus::InReview => &by_status.inreview,
            TaskStatus::Done => &by_status.done,
            TaskStatus::Cancelled => &by_status.cancelled,
        };

        let is_active = status == app.board.tasks_active_column;
        let border_style = if app.ui.focus == FocusPane::Board && is_active {
            Style::default().fg(Color::Cyan)
        } else if app.ui.focus == FocusPane::Board {
            Style::default().fg(Color::Gray)
        } else {
            Style::default()
        };

        let title = format!("{} ({})", status.label(), list.len());

        let height = sections[idx].height.saturating_sub(2) as usize;
        let (items, selected_in_window) = if list.is_empty() || height == 0 {
            (vec![ListItem::new(Line::from("—"))], None)
        } else {
            let selected_idx = if is_active {
                task_index_in(list, app.board.selected_task_id)
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
            let (start, end, selected_in_window) = window_for_list(list.len(), selected_idx, height);
            let visible = &list[start..end];
            let items = if visible.is_empty() {
                vec![ListItem::new(Line::from("—"))]
            } else {
                visible
                    .iter()
                    .map(|t| ListItem::new(render_task_line(t)))
                    .collect()
            };
            (items, is_active.then_some(selected_in_window))
        };

        let widget = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(border_style),
            )
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(if is_active { "▶ " } else { "  " });

        let mut state = ratatui::widgets::ListState::default();
        state.select(selected_in_window);
        f.render_stateful_widget(widget, sections[idx], &mut state);
    }
}
