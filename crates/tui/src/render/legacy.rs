#![allow(dead_code)]

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::{
    events::StreamStatus,
    fmt::short_time,
    selection::{
        exec_list, filtered_projects, find_task, projects_list, task_index_in, tasks_by_status,
        tasks_filtered_base,
    },
    state::{AppState, FocusPane, TaskRow, TaskStatus},
};

pub(crate) fn render_projects_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let border_style = Style::default();

    let projects = filtered_projects(app);
    let items: Vec<ListItem> = if projects.is_empty() {
        vec![ListItem::new(Line::from("No projects"))]
    } else {
        projects
            .iter()
            .map(|p| ListItem::new(Line::from(p.name.clone())))
            .collect()
    };

    let title = format!(
        "Projects ({}){}",
        match app.board.projects_status {
            StreamStatus::Connected => "live",
            StreamStatus::Connecting => "connecting",
            StreamStatus::Completed => "done",
            StreamStatus::Disconnected => "offline",
            StreamStatus::Error => "error",
        },
        if app.board.project_filter.trim().is_empty() {
            "".to_string()
        } else {
            format!(" /{}", app.board.project_filter.trim())
        }
    );

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");

    let mut state = ratatui::widgets::ListState::default();
    if !projects.is_empty() {
        let idx = app
            .board
            .selected_project_id
            .and_then(|id| projects.iter().position(|p| p.id == id))
            .unwrap_or(0);
        state.select(Some(idx));
    }
    f.render_stateful_widget(list, area, &mut state);
}

pub(crate) fn render_tasks_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    render_tasks_board(f, app, area)
}

pub(crate) fn render_tasks_board(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let tasks = tasks_filtered_base(app);
    let by_status = tasks_by_status(&tasks);
    let columns: Vec<TaskStatus> = if app.board.show_cancelled {
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
    };

    let pct = (100 / columns.len().max(1)) as u16;
    let constraints: Vec<Constraint> = (0..columns.len())
        .map(|_| Constraint::Percentage(pct))
        .collect();

    let col_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, status) in columns.into_iter().enumerate() {
        let list: &[TaskRow] = match status {
            TaskStatus::Todo => &by_status.todo,
            TaskStatus::InProgress => &by_status.inprogress,
            TaskStatus::InReview => &by_status.inreview,
            TaskStatus::Done => &by_status.done,
            TaskStatus::Cancelled => &by_status.cancelled,
        };

        let is_active_col =
            app.ui.focus == FocusPane::Board && app.board.tasks_active_column == status;
        let border_style = if is_active_col {
            Style::default().fg(Color::Cyan)
        } else if app.ui.focus == FocusPane::Board {
            Style::default()
        } else {
            Style::default()
        };

        let title = format!("{} ({})", status.label(), list.len());
        let items: Vec<ListItem> = if list.is_empty() {
            vec![ListItem::new(Line::from("—"))]
        } else {
            list.iter()
                .map(|t| ListItem::new(crate::ui::components::task_lines::render_task_line(t)))
                .collect()
        };

        let widget = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(border_style),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▶ ");

        let mut state = ratatui::widgets::ListState::default();
        if let Some(idx) = task_index_in(list, app.board.selected_task_id) {
            state.select(Some(idx));
        } else if is_active_col && !list.is_empty() {
            state.select(Some(0));
        }

        f.render_stateful_widget(widget, col_areas[i], &mut state);
    }
}

pub(crate) fn render_tasks_table(f: &mut Frame, _app: &AppState, area: ratatui::layout::Rect) {
    let w = Paragraph::new("Table view removed (use the board)")
        .block(Block::default().borders(Borders::ALL).title("Tasks"));
    f.render_widget(w, area);
}

pub(crate) fn render_details_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Min(5),
            Constraint::Min(5),
        ])
        .split(area);

    let project_name = app.board.selected_project_id.and_then(|id| {
        projects_list(&app.board.projects_store)
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
    });

    let mut header_lines: Vec<Line<'static>> = vec![];
    header_lines.push(Line::from(format!(
        "project: {}",
        project_name.unwrap_or_else(|| "none".to_string())
    )));

    match app
        .board
        .selected_task_id
        .and_then(|id| find_task(&app.board.tasks_store, id))
    {
        Some(task) => header_lines.push(Line::from(format!("task: {}", task.title))),
        None => header_lines.push(Line::from("task: none")),
    }
    header_lines.push(Line::from(app.info_summary.clone()));

    let header = Paragraph::new(header_lines)
        .block(Block::default().borders(Borders::ALL).title("Selection"))
        .wrap(Wrap { trim: true });
    f.render_widget(header, sections[0]);

    let attempts_border = Style::default();

    let attempts_items: Vec<ListItem> = if app.board.attempts.is_empty() {
        vec![ListItem::new(Line::from("No attempts"))]
    } else {
        app.board
            .attempts
            .iter()
            .map(|a| {
                let when = a.created_at.as_deref().and_then(short_time).unwrap_or("");
                let updated = a.updated_at.as_deref().and_then(short_time).unwrap_or("");
                let suffix = if a.setup_completed_at.is_some() {
                    " setup✓"
                } else {
                    ""
                };
                let text = if when.is_empty() && updated.is_empty() {
                    format!("{}{}", a.branch, suffix)
                } else if updated.is_empty() {
                    format!("{} [{}]{}", a.branch, when, suffix)
                } else if when.is_empty() {
                    format!("{} [u:{}]{}", a.branch, updated, suffix)
                } else {
                    format!("{} [{} u:{}]{}", a.branch, when, updated, suffix)
                };
                ListItem::new(Line::from(text))
            })
            .collect()
    };
    let attempts_title = format!("Attempts ({})", app.board.attempts.len());
    let attempts_list = List::new(attempts_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(attempts_title)
                .border_style(attempts_border),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    let mut attempts_state = ratatui::widgets::ListState::default();
    if !app.board.attempts.is_empty() {
        attempts_state.select(Some(
            app.board
                .selected_attempt_index
                .min(app.board.attempts.len() - 1),
        ));
    }
    f.render_stateful_widget(attempts_list, sections[1], &mut attempts_state);

    let execs_border = Style::default();

    let execs = exec_list(&app.exec.exec_store);
    let exec_items: Vec<ListItem> = if execs.is_empty() {
        vec![ListItem::new(Line::from("No execution processes"))]
    } else {
        execs
            .iter()
            .map(|e| {
                let reason = e.run_reason.clone().unwrap_or_else(|| "?".to_string());
                let status = e.status.clone().unwrap_or_else(|| "?".to_string());
                let dropped = if e.dropped { " dropped" } else { "" };
                ListItem::new(Line::from(format!("{reason}: {status}{dropped}")))
            })
            .collect()
    };
    let exec_title = format!(
        "Execs ({}) {}",
        execs.len(),
        match app.exec.exec_status {
            StreamStatus::Connected => "live",
            StreamStatus::Connecting => "connecting",
            StreamStatus::Completed => "done",
            StreamStatus::Disconnected => "offline",
            StreamStatus::Error => "error",
        }
    );
    let exec_list_widget = List::new(exec_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(exec_title)
                .border_style(execs_border),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    let mut exec_state = ratatui::widgets::ListState::default();
    if !execs.is_empty() {
        let idx = app
            .exec
            .selected_exec_id
            .and_then(|id| execs.iter().position(|e| e.id == id))
            .unwrap_or(0);
        exec_state.select(Some(idx));
    }
    f.render_stateful_widget(exec_list_widget, sections[2], &mut exec_state);
}

pub(crate) fn render_logs_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let border_style = if app.ui.focus == FocusPane::Execution {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let err_line = app.ui.last_error.as_ref().map(|e| {
        Line::from(vec![Span::styled(
            e.clone(),
            Style::default().fg(Color::Red),
        )])
    });

    let max_render = 200usize;
    let len = app.exec.log_lines.len();
    let visible = max_render.min(len);
    let mut offset = if app.exec.log_autoscroll {
        0
    } else {
        app.exec.log_scroll_offset
    };
    offset = offset.min(len.saturating_sub(visible));
    let start = len.saturating_sub(visible + offset);
    let end = len.saturating_sub(offset);

    let mut text: Vec<Line<'static>> = app.exec.log_lines.get(start..end).unwrap_or(&[]).to_vec();
    if text.is_empty() {
        text.push(Line::from("No logs"));
    }
    if let Some(line) = err_line {
        text.push(Line::from(""));
        text.push(Line::from("Last error:"));
        text.push(line);
    }

    let title = format!(
        "Logs ({}, {}, {})",
        match app.exec.log_status {
            StreamStatus::Connected => "live",
            StreamStatus::Connecting => "connecting",
            StreamStatus::Completed => "done",
            StreamStatus::Disconnected => "offline",
            StreamStatus::Error => "error",
        },
        app.exec.log_mode.label(),
        app.exec.log_render_mode.label()
    );
    let p = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}
