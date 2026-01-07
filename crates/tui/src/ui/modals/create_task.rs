use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    selection::{find_task, projects_list},
    state::{AppState, CreateTaskFocus, CreateTaskState, TaskStatus},
    ui::layout::centered_rect,
};

pub(crate) fn open_create_task_modal(app: &mut AppState, parent_task_id: Option<uuid::Uuid>) {
    app.ui.create_task = Some(CreateTaskState {
        title: Default::default(),
        description: Default::default(),
        status: TaskStatus::Todo,
        parent_task_id,
        focus: CreateTaskFocus::Title,
        selected_button: 0,
        error: None,
    });
}

pub(crate) fn handle_create_task_key(app: &mut AppState, key: crossterm::event::KeyEvent) -> bool {
    let Some(mut state) = app.ui.create_task.take() else {
        return false;
    };
    state.error = None;

    let mut close = false;
    let mut submit = false;

    let can_create =
        app.board.selected_project_id.is_some() && !state.title.buffer.trim().is_empty();

    use crossterm::event::KeyCode;
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => {
            close = true;
        }
        (KeyCode::Tab, _) => {
            state.focus = next_focus(state.focus);
        }
        (KeyCode::BackTab, _) => {
            state.focus = prev_focus(state.focus);
        }
        _ => match state.focus {
            CreateTaskFocus::Title => {
                if matches!(key.code, KeyCode::Enter) {
                    state.focus = CreateTaskFocus::Description;
                } else {
                    let _ =
                        crate::text::field_edit::apply_text_field_key(&mut state.title, key, false);
                }
            }
            CreateTaskFocus::Description => {
                let _ = crate::text::field_edit::apply_text_field_key(
                    &mut state.description,
                    key,
                    true,
                );
            }
            CreateTaskFocus::Status => match (key.code, key.modifiers) {
                (KeyCode::Left, _) | (KeyCode::Char('h'), _) => {
                    state.status = status_cycle(state.status, -1)
                }
                (KeyCode::Right, _) | (KeyCode::Char('l'), _) => {
                    state.status = status_cycle(state.status, 1)
                }
                (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                    state.status = status_cycle(state.status, -1)
                }
                (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                    state.status = status_cycle(state.status, 1)
                }
                (KeyCode::Enter, _) => state.focus = CreateTaskFocus::Buttons,
                _ => {}
            },
            CreateTaskFocus::Buttons => match (key.code, key.modifiers) {
                (KeyCode::Left, _) | (KeyCode::Char('h'), _) => state.selected_button = 0,
                (KeyCode::Right, _) | (KeyCode::Char('l'), _) => state.selected_button = 1,
                (KeyCode::Enter, _) => {
                    if state.selected_button == 1 {
                        close = true;
                    } else if can_create {
                        submit = true;
                    } else {
                        state.error = if app.board.selected_project_id.is_none() {
                            Some("Project is required.".to_string())
                        } else {
                            Some("Title is required.".to_string())
                        };
                    }
                }
                _ => {}
            },
        },
    }

    ensure_cursor_visible(&mut state);

    if close {
        return true;
    }

    if submit {
        submit_create_task_state(app, state);
        return true;
    }

    app.ui.create_task = Some(state);
    true
}

fn create_task_focus_border(focus: CreateTaskFocus, current: CreateTaskFocus) -> Style {
    if focus == current {
        crate::ui::palette::border_active()
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

    let parent_label = state
        .parent_task_id
        .and_then(|id| find_task(&app.board.tasks_store, id))
        .map(|t| t.title)
        .unwrap_or_else(|| "none".to_string());

    let inner = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            "Create Task  (project: {project_label}, parent: {parent_label})"
        ))
        .border_style(crate::ui::palette::border_active());

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
    let (desc_scroll_y, desc_scroll_x) = state
        .description
        .ensured_scroll(desc_inner_w.max(1), desc_inner_h.max(1));
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
            st = st
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD);
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
        btn_line.push(Span::styled(
            err.clone(),
            Style::default().fg(crate::ui::palette::error_fg()),
        ));
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
                .saturating_add(cursor_col.saturating_sub(title_scroll_x as usize) as u16)
                .min(
                    chunks[0]
                        .x
                        .saturating_add(chunks[0].width)
                        .saturating_sub(2),
                );
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
                .min(
                    chunks[1]
                        .x
                        .saturating_add(chunks[1].width)
                        .saturating_sub(2),
                );
            let y = chunks[1]
                .y
                .saturating_add(1)
                .saturating_add(vy.min(inner_h.saturating_sub(1)) as u16);
            f.set_cursor_position((x, y));
        }
        _ => {}
    }
}

fn status_cycle(status: TaskStatus, dir: i32) -> TaskStatus {
    let all = [
        TaskStatus::Todo,
        TaskStatus::InProgress,
        TaskStatus::InReview,
        TaskStatus::Done,
        TaskStatus::Cancelled,
    ];
    let idx = all.iter().position(|s| *s == status).unwrap_or(0);
    if dir < 0 {
        all[idx.checked_sub(1).unwrap_or(0)]
    } else {
        all.get(idx + 1).copied().unwrap_or(all[all.len() - 1])
    }
}

fn next_focus(f: CreateTaskFocus) -> CreateTaskFocus {
    match f {
        CreateTaskFocus::Title => CreateTaskFocus::Description,
        CreateTaskFocus::Description => CreateTaskFocus::Status,
        CreateTaskFocus::Status => CreateTaskFocus::Buttons,
        CreateTaskFocus::Buttons => CreateTaskFocus::Title,
    }
}

fn prev_focus(f: CreateTaskFocus) -> CreateTaskFocus {
    match f {
        CreateTaskFocus::Title => CreateTaskFocus::Buttons,
        CreateTaskFocus::Description => CreateTaskFocus::Title,
        CreateTaskFocus::Status => CreateTaskFocus::Description,
        CreateTaskFocus::Buttons => CreateTaskFocus::Status,
    }
}

fn create_task_modal_desc_inner_dims(term: ratatui::layout::Rect) -> (usize, usize) {
    // Keep this in sync with `render_create_task_modal`.
    let area = crate::ui::layout::centered_rect(75, 80, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2) as usize;
    // Layout in render_create_task_modal:
    // - Title field: 3
    // - Description: min 10 (rest)
    // - Status: 3
    // - Buttons: 4
    let desc_inner_w = inner_w.saturating_sub(2).max(1);
    let desc_inner_h = inner_h
        .saturating_sub(3) // title
        .saturating_sub(3) // status
        .saturating_sub(4) // buttons
        .saturating_sub(2) // description borders
        .max(1);
    (desc_inner_w, desc_inner_h)
}

fn create_task_modal_title_inner_w(term: ratatui::layout::Rect) -> usize {
    let area = crate::ui::layout::centered_rect(75, 80, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    inner_w.saturating_sub(2).max(1)
}

fn ensure_cursor_visible(state: &mut CreateTaskState) {
    if state.focus == CreateTaskFocus::Description {
        let (inner_w, inner_h) =
            create_task_modal_desc_inner_dims(crate::layout::current_terminal_rect());
        state.description.ensure_cursor_visible(inner_w, inner_h);
    } else if state.focus == CreateTaskFocus::Title {
        let inner_w = create_task_modal_title_inner_w(crate::layout::current_terminal_rect());
        state.title.ensure_cursor_visible(inner_w.max(1), 1);
    }
}

fn submit_create_task_state(app: &mut AppState, state: CreateTaskState) {
    let Some(project_id) = app.board.selected_project_id else {
        app.ui.create_task = Some(CreateTaskState {
            error: Some("No project selected.".to_string()),
            ..state
        });
        return;
    };
    let title = state.title.buffer.trim().to_string();
    if title.is_empty() {
        app.ui.create_task = Some(CreateTaskState {
            error: Some("Title is required.".to_string()),
            ..state
        });
        return;
    }
    let description = state
        .description
        .buffer
        .trim_end_matches('\n')
        .trim()
        .to_string();
    let description = (!description.is_empty()).then_some(description);
    let status = state.status;
    let parent_task_id = state.parent_task_id;

    app.ui.set_toast(
        "Creating task…",
        crate::ui::palette::toast_info(),
        Some(std::time::Duration::from_secs(2)),
    );

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match crate::net::ops::create_task_http(
            &base_url,
            project_id,
            &title,
            description.as_deref(),
            status,
            parent_task_id,
        )
        .await
        {
            Ok(task_id) => {
                let _ = net_tx
                    .send(crate::events::NetEvent::TaskCreated { task_id, status })
                    .await;
                let _ = net_tx
                    .send(crate::events::NetEvent::Notice(format!(
                        "Created task: {title}"
                    )))
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(crate::events::NetEvent::Error(format!(
                        "create task failed: {e}"
                    )))
                    .await;
            }
        }
    });
}
