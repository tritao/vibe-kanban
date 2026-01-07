use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
};

use super::UiComponent;
use crate::{
    layout::rect_contains,
    prefs::save_prefs,
    selection::{BoardTaskItem, board_tasks_by_status, find_task},
    state::{
        AppState, ConfirmAction, ConfirmAltAction, ConfirmState, DeleteTaskMode, FocusPane,
        TaskStatus,
    },
    ui::components::task_lines::render_board_task_line,
    util::{board_statuses, window_for_list},
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct BoardHit {
    pub(crate) status: TaskStatus,
    pub(crate) clicked_index: Option<usize>,
    pub(crate) clicked_task_id: Option<uuid::Uuid>,
}

pub(crate) enum BoardPaneEvent {
    Key(KeyEvent),
    Click(BoardHit),
    Wheel { status: TaskStatus, delta: i32 },
}

pub(crate) struct BoardPane;

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

fn task_index_in(list: &[BoardTaskItem], task_id: Option<uuid::Uuid>) -> Option<usize> {
    let id = task_id?;
    list.iter().position(|t| t.task.id == id)
}

pub(crate) fn board_hit_at(app: &AppState, area: Rect, col: u16, row: u16) -> Option<BoardHit> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let by_status = board_tasks_by_status(app);
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
        let clicked_task_id = list.get(clicked_index).map(|t| t.task.id);
        return Some(BoardHit {
            status,
            clicked_index: Some(clicked_index),
            clicked_task_id,
        });
    }

    None
}

pub(crate) fn render_board_pane(f: &mut Frame, app: &AppState, area: Rect) {
    let by_status = board_tasks_by_status(app);
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

        let list: &[BoardTaskItem] = match status {
            TaskStatus::Todo => &by_status.todo,
            TaskStatus::InProgress => &by_status.inprogress,
            TaskStatus::InReview => &by_status.inreview,
            TaskStatus::Done => &by_status.done,
            TaskStatus::Cancelled => &by_status.cancelled,
        };

        let is_active = status == app.board.tasks_active_column;
        let border_style = if app.ui.focus == FocusPane::Board && is_active {
            crate::ui::palette::border_active()
        } else if app.ui.focus == FocusPane::Board {
            crate::ui::palette::border_inactive()
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
            let (start, end, selected_in_window) =
                window_for_list(list.len(), selected_idx, height);
            let visible = &list[start..end];
            let items = if visible.is_empty() {
                vec![ListItem::new(Line::from("—"))]
            } else {
                visible
                    .iter()
                    .map(|t| ListItem::new(render_board_task_line(t)))
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

impl BoardPane {
    fn open_delete_task_confirm(app: &mut AppState) {
        let Some(task_id) = app.board.selected_task_id else {
            app.ui.set_error("No task selected.");
            return;
        };
        let title = find_task(&app.board.tasks_store, task_id)
            .map(|t| t.title)
            .unwrap_or_else(|| task_id.to_string());

        let all_tasks = crate::selection::tasks_all(&app.board.tasks_store);
        let mut children_by_parent: std::collections::HashMap<uuid::Uuid, Vec<uuid::Uuid>> =
            std::collections::HashMap::new();
        for t in &all_tasks {
            if let Some(pid) = t.parent_task_id {
                children_by_parent.entry(pid).or_default().push(t.id);
            }
        }

        let mut descendants = 0usize;
        let mut stack = children_by_parent
            .get(&task_id)
            .cloned()
            .unwrap_or_default();
        while let Some(cur) = stack.pop() {
            descendants += 1;
            if let Some(ch) = children_by_parent.get(&cur) {
                stack.extend(ch.iter().copied());
            }
        }

        let body = if descendants > 0 {
            format!(
                "Delete task '{title}'?\n\nThis task has {descendants} subtask(s).\n\n- y/Enter: delete (promote subtasks)\n- D: delete subtree (destructive)"
            )
        } else {
            format!("Delete task '{title}'?")
        };

        app.ui.confirm = Some(ConfirmState {
            title: "Delete task?".to_string(),
            body,
            action: ConfirmAction::DeleteTask {
                task_id,
                delete_mode: DeleteTaskMode::Promote,
            },
            alt_action: (descendants > 0).then_some(ConfirmAltAction {
                key: 'D',
                label: "delete subtree".to_string(),
                action: ConfirmAction::DeleteTask {
                    task_id,
                    delete_mode: DeleteTaskMode::Subtree,
                },
            }),
        });
    }
}

impl UiComponent for BoardPane {
    type Event = BoardPaneEvent;

    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        render_board_pane(f, app, area);
    }

    fn hit_test(app: &AppState, area: Rect, col: u16, row: u16) -> Option<Self::Event> {
        board_hit_at(app, area, col, row).map(BoardPaneEvent::Click)
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        match event {
            BoardPaneEvent::Click(hit) => {
                if app.ui.focus != FocusPane::Board {
                    return false;
                }
                crate::actions::selection::apply_board_hit(app, hit);
                true
            }
            BoardPaneEvent::Wheel { status, delta } => {
                if app.ui.focus != FocusPane::Board {
                    return false;
                }
                let delta = delta.signum();
                if delta == 0 {
                    return false;
                }
                crate::actions::selection::focus_board_section(app, status);
                crate::actions::selection::select_adjacent_task(app, delta);
                true
            }
            BoardPaneEvent::Key(key) => {
                if app.ui.focus != FocusPane::Board {
                    return false;
                }
                match key.code {
                    KeyCode::Char('c') => {
                        app.board.show_cancelled = !app.board.show_cancelled;
                        crate::actions::selection::normalize_after_cancelled_toggle(app);
                        app.prefs.show_cancelled = app.board.show_cancelled;
                        save_prefs(&app.prefs);
                        true
                    }
                    KeyCode::Char('n') => {
                        crate::ui::open_create_task_modal(app, None);
                        true
                    }
                    KeyCode::Char('N') => {
                        crate::ui::open_create_task_modal(app, app.board.selected_task_id);
                        true
                    }
                    KeyCode::Char('d') => {
                        BoardPane::open_delete_task_confirm(app);
                        true
                    }
                    KeyCode::Char('K') => {
                        crate::actions::selection::move_active_status(app, -1);
                        true
                    }
                    KeyCode::Char('J') => {
                        crate::actions::selection::move_active_status(app, 1);
                        true
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        crate::actions::selection::select_adjacent_task(app, -1);
                        true
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        crate::actions::selection::select_adjacent_task(app, 1);
                        true
                    }
                    KeyCode::Left => {
                        crate::actions::selection::request_move_selected_task(app, -1);
                        true
                    }
                    KeyCode::Right => {
                        crate::actions::selection::request_move_selected_task(app, 1);
                        true
                    }
                    _ => false,
                }
            }
        }
    }
}
