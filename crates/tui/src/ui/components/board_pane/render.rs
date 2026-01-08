use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem},
};

use crate::{
    selection::BoardTaskItem,
    state::{AppState, FocusPane, TaskStatus},
    store::tasks_board::board_tasks_by_status,
    ui::components::task_lines::render_board_task_line,
    util::{board_statuses, window_for_list},
};

pub(super) fn render_board_pane(f: &mut Frame, app: &AppState, area: Rect) {
    let by_status = board_tasks_by_status(&app.board.tasks_store, &app.board.task_filter);
    let statuses = board_statuses(app);
    let needs = super::layout::board_section_needs(app);
    let heights = super::layout::allocate_board_section_heights(&needs, area.height);
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
