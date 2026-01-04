use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::layout::centered_rect;
use crate::selection::projects_list;
use crate::state::{AppState, CreateTaskFocus, CreateTaskState, TaskStatus};

pub(crate) fn open_create_task_modal(app: &mut AppState) {
    app.ui.create_task = Some(CreateTaskState {
        title: Default::default(),
        description: Default::default(),
        status: TaskStatus::Todo,
        focus: CreateTaskFocus::Title,
        selected_button: 0,
        error: None,
    });
}

fn create_task_focus_border(focus: CreateTaskFocus, current: CreateTaskFocus) -> Style {
    if focus == current {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}

pub(crate) fn render_create_task_modal(f: &mut Frame, app: &AppState, state: &CreateTaskState) {
    // Slightly taller modal so the description editor can be more comfortable.
    let area = centered_rect(75, 80, f.area());
    f.render_widget(Clear, area);

    let project_name = app.board.selected_project_id.and_then(|id| {
        projects_list(&app.board.projects_store)
            .into_iter()
            .find(|p| p.id == id)
            .map(|p| p.name)
    });
    let project_label = project_name.unwrap_or_else(|| "none".to_string());

    let inner = Block::default()
        .borders(Borders::ALL)
        .title(format!("Create Task  (project: {project_label})"))
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = inner.inner(area);
    f.render_widget(inner, area);

    let constraints = [
        Constraint::Length(3), // title
        Constraint::Min(10),   // description (expands as space allows)
        Constraint::Length(3), // status
        Constraint::Length(4), // buttons + hints
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner_area);

    // Title
    let title_border = create_task_focus_border(state.focus, CreateTaskFocus::Title);
    let title_inner_w = chunks[0].width.saturating_sub(2) as usize;
    let (_, title_scroll_x) = state.title.ensured_scroll(title_inner_w.max(1), 1);
    let title_p = Paragraph::new(Line::from(state.title.buffer.clone()))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Title")
                .border_style(title_border),
        )
        .wrap(Wrap { trim: false });
    let title_p = title_p.scroll((0, title_scroll_x));
    f.render_widget(title_p, chunks[0]);

    // Description (auto-scroll to keep the cursor visible)
    let desc_border = create_task_focus_border(state.focus, CreateTaskFocus::Description);
    let desc_inner_h = chunks[1].height.saturating_sub(2) as usize;
    let desc_inner_w = chunks[1].width.saturating_sub(2) as usize;
    let (desc_scroll_y, desc_scroll_x) =
        state.description.ensured_scroll(desc_inner_w.max(1), desc_inner_h.max(1));
    let desc_lines: Vec<Line<'static>> = if state.description.buffer.is_empty() {
        vec![Line::from(Span::styled(
            "Optional. Markdown supported.",
            Style::default().add_modifier(Modifier::DIM),
        ))]
    } else {
        state
            .description
            .buffer
            .split('\n')
            .map(|l| Line::from(l.to_string()))
            .collect()
    };
    let mut desc_p = Paragraph::new(desc_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Description")
            .border_style(desc_border),
    );
    if !state.description.buffer.is_empty() {
        desc_p = desc_p.scroll((desc_scroll_y, desc_scroll_x));
    }
    f.render_widget(desc_p, chunks[1]);

    // Status
    let status_border = create_task_focus_border(state.focus, CreateTaskFocus::Status);
    let statuses = [
        TaskStatus::Todo,
        TaskStatus::InProgress,
        TaskStatus::InReview,
        TaskStatus::Done,
        TaskStatus::Cancelled,
    ];
    let mut status_spans: Vec<Span<'static>> = vec![];
    for (i, s) in statuses.into_iter().enumerate() {
        if i > 0 {
            status_spans.push(Span::raw("  "));
        }
        let mut st = Style::default();
        if s == state.status {
            st = st.add_modifier(Modifier::REVERSED).add_modifier(Modifier::BOLD);
        } else {
            st = st.add_modifier(Modifier::DIM);
        }
        status_spans.push(Span::styled(s.label().to_string(), st));
    }
    let status_p = Paragraph::new(Line::from(status_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Status")
            .border_style(status_border),
    );
    f.render_widget(status_p, chunks[2]);

    // Buttons + hints
    let buttons_border = create_task_focus_border(state.focus, CreateTaskFocus::Buttons);
    let can_create =
        !state.title.buffer.trim().is_empty() && app.board.selected_project_id.is_some();
    let create_style = if state.selected_button == 0 {
        if can_create {
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::DIM)
        }
    } else if can_create {
        Style::default()
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };
    let cancel_style = if state.selected_button == 1 {
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let mut btn_line: Vec<Span<'static>> = vec![
        Span::styled(" Create ", create_style),
        Span::raw(" "),
        Span::styled(" Cancel ", cancel_style),
    ];
    if let Some(err) = state.error.as_ref().filter(|s| !s.trim().is_empty()) {
        btn_line.push(Span::raw("  "));
        btn_line.push(Span::styled(err.clone(), Style::default().fg(Color::Red)));
    } else if app.board.selected_project_id.is_none() {
        btn_line.push(Span::raw("  "));
        btn_line.push(Span::styled(
            "Project required",
            Style::default().add_modifier(Modifier::DIM),
        ));
    } else if state.title.buffer.trim().is_empty() {
        btn_line.push(Span::raw("  "));
        btn_line.push(Span::styled(
            "Title required",
            Style::default().add_modifier(Modifier::DIM),
        ));
    }

    let hints = Line::from(Span::styled(
        "Tab: next field  Ctrl+Enter: create  Esc: cancel",
        Style::default().add_modifier(Modifier::DIM),
    ));
    let buttons_p = Paragraph::new(vec![Line::from(btn_line), hints]).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Actions")
            .border_style(buttons_border),
    );
    f.render_widget(buttons_p, chunks[3]);

    // Cursor positioning (end of current field, approximate).
    let cursor_pad_x = 1u16;
    match state.focus {
        CreateTaskFocus::Title => {
            let (_, cursor_col) = state.title.cursor_line_col();
            let x = chunks[0]
                .x
                .saturating_add(cursor_pad_x)
                .saturating_add(
                    cursor_col.saturating_sub(title_scroll_x as usize) as u16
                )
                .min(chunks[0].x.saturating_add(chunks[0].width).saturating_sub(2));
            let y = chunks[0].y.saturating_add(1);
            f.set_cursor_position((x, y));
        }
        CreateTaskFocus::Description => {
            let inner_w = chunks[1].width.saturating_sub(2) as usize;
            let inner_h = chunks[1].height.saturating_sub(2) as usize;
            if inner_w == 0 || inner_h == 0 {
                return;
            }
            if state.description.buffer.is_empty() {
                let x = chunks[1].x.saturating_add(cursor_pad_x);
                let y = chunks[1].y.saturating_add(1);
                f.set_cursor_position((x, y));
                return;
            }
            let (line, col) = state.description.cursor_line_col();
            let vx = col.saturating_sub(desc_scroll_x as usize);
            let vy = line.saturating_sub(desc_scroll_y as usize);
            let x = chunks[1]
                .x
                .saturating_add(cursor_pad_x)
                .saturating_add(vx.min(inner_w.saturating_sub(1)) as u16)
                .min(chunks[1].x.saturating_add(chunks[1].width).saturating_sub(2));
            let y = chunks[1]
                .y
                .saturating_add(1)
                .saturating_add(vy.min(inner_h.saturating_sub(1)) as u16);
            f.set_cursor_position((x, y));
        }
        _ => {}
    }
}
