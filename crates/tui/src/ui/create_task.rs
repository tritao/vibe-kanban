use std::time::{Duration, Instant};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::layout::centered_rect;
use crate::commands::set_toast;
use crate::net::ops::create_task_http;
use crate::layout::current_terminal_rect;
use crate::selection::projects_list;
use crate::events::NetEvent;
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

fn create_task_next_focus(focus: CreateTaskFocus) -> CreateTaskFocus {
    match focus {
        CreateTaskFocus::Title => CreateTaskFocus::Description,
        CreateTaskFocus::Description => CreateTaskFocus::Status,
        CreateTaskFocus::Status => CreateTaskFocus::Buttons,
        CreateTaskFocus::Buttons => CreateTaskFocus::Title,
    }
}

fn create_task_prev_focus(focus: CreateTaskFocus) -> CreateTaskFocus {
    match focus {
        CreateTaskFocus::Title => CreateTaskFocus::Buttons,
        CreateTaskFocus::Description => CreateTaskFocus::Title,
        CreateTaskFocus::Status => CreateTaskFocus::Description,
        CreateTaskFocus::Buttons => CreateTaskFocus::Status,
    }
}

fn create_task_modal_desc_inner_dims(term: Rect) -> (usize, usize) {
    let area = centered_rect(75, 70, term);
    let inner = Block::default().borders(Borders::ALL).inner(area);
    let constraints = [
        Constraint::Length(3), // title
        Constraint::Length(7), // description
        Constraint::Length(3), // status
        Constraint::Length(3), // buttons
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);
    let desc = chunks[1];
    let inner_w = desc.width.saturating_sub(2) as usize;
    let inner_h = desc.height.saturating_sub(2) as usize;
    (inner_w.max(1), inner_h.max(1))
}

fn create_task_modal_title_inner_w(term: Rect) -> usize {
    let area = centered_rect(75, 70, term);
    let inner = Block::default().borders(Borders::ALL).inner(area);
    let constraints = [
        Constraint::Length(3), // title
        Constraint::Length(7), // description
        Constraint::Length(3), // status
        Constraint::Length(3), // buttons
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);
    let title = chunks[0];
    title.width.saturating_sub(2) as usize
}

pub(crate) fn handle_create_task_key(app: &mut AppState, key: crossterm::event::KeyEvent) {
    use crossterm::event::{KeyCode, KeyModifiers};

    let Some(mut state) = app.ui.create_task.take() else {
        return;
    };
    state.error = None;

    let can_create =
        !state.title.buffer.trim().is_empty() && app.board.selected_project_id.is_some();
    let mut close = false;
    let mut submit = false;

    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => {
            close = true;
        }
        (KeyCode::Tab, KeyModifiers::SHIFT) => {
            state.focus = create_task_prev_focus(state.focus);
        }
        (KeyCode::Tab, _) => {
            state.focus = create_task_next_focus(state.focus);
        }
        (KeyCode::Enter, KeyModifiers::CONTROL) => {
            if can_create {
                submit = true;
            } else if app.board.selected_project_id.is_none() {
                state.error = Some("Project is required.".to_string());
            } else {
                state.error = Some("Title is required.".to_string());
            }
        }
        _ => match state.focus {
            CreateTaskFocus::Title => match (key.code, key.modifiers) {
                (KeyCode::Enter, _) => {
                    state.focus = CreateTaskFocus::Description;
                }
                (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
                    state.title.undo();
                }
                (KeyCode::Char('y'), KeyModifiers::CONTROL)
                | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                    state.title.redo();
                }
                (KeyCode::Left, KeyModifiers::CONTROL) => {
                    state.title.move_word_left();
                }
                (KeyCode::Right, KeyModifiers::CONTROL) => {
                    state.title.move_word_right();
                }
                (KeyCode::Left, _) => {
                    state.title.move_left();
                }
                (KeyCode::Right, _) => {
                    state.title.move_right();
                }
                (KeyCode::Home, _) => {
                    state.title.move_home(false);
                }
                (KeyCode::End, _) => {
                    state.title.move_end(false);
                }
                (KeyCode::Backspace, KeyModifiers::ALT)
                | (KeyCode::Backspace, KeyModifiers::CONTROL) => {
                    state.title.backspace_word();
                }
                (KeyCode::Backspace, _) => {
                    state.title.backspace();
                }
                (KeyCode::Delete, KeyModifiers::CONTROL) => {
                    state.title.delete_word();
                }
                (KeyCode::Delete, _) => {
                    state.title.delete();
                }
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                    state.title.clear();
                }
                (KeyCode::Char(c), KeyModifiers::NONE) => {
                    state.title.insert_char(c);
                }
                _ => {}
            },
            CreateTaskFocus::Description => match (key.code, key.modifiers) {
                (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
                    state.description.undo();
                }
                (KeyCode::Char('y'), KeyModifiers::CONTROL)
                | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
                    state.description.redo();
                }
                (KeyCode::Left, KeyModifiers::CONTROL) => {
                    state.description.move_word_left();
                }
                (KeyCode::Right, KeyModifiers::CONTROL) => {
                    state.description.move_word_right();
                }
                (KeyCode::Backspace, KeyModifiers::ALT)
                | (KeyCode::Backspace, KeyModifiers::CONTROL) => {
                    state.description.backspace_word();
                }
                (KeyCode::Delete, KeyModifiers::CONTROL) => {
                    state.description.delete_word();
                }
                (KeyCode::Backspace, _) => {
                    state.description.backspace();
                }
                (KeyCode::Delete, _) => {
                    state.description.delete();
                }
                (KeyCode::Left, _) => {
                    state.description.move_left();
                }
                (KeyCode::Right, _) => {
                    state.description.move_right();
                }
                (KeyCode::Up, _) => {
                    state.description.move_up();
                }
                (KeyCode::Down, _) => {
                    state.description.move_down();
                }
                (KeyCode::Home, _) => {
                    state.description.move_home(true);
                }
                (KeyCode::End, _) => {
                    state.description.move_end(true);
                }
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                    state.description.clear();
                }
                (KeyCode::Enter, _) => {
                    state.description.insert_char('\n');
                }
                (KeyCode::Char(c), KeyModifiers::NONE) => {
                    state.description.insert_char(c);
                }
                _ => {}
            },
            CreateTaskFocus::Status => match (key.code, key.modifiers) {
                (KeyCode::Left, _) | (KeyCode::Char('h'), _) => {
                    state.status = status_cycle(state.status, -1);
                }
                (KeyCode::Right, _) | (KeyCode::Char('l'), _) => {
                    state.status = status_cycle(state.status, 1);
                }
                (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                    state.status = status_cycle(state.status, -1);
                }
                (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                    state.status = status_cycle(state.status, 1);
                }
                (KeyCode::Enter, _) => {
                    state.focus = CreateTaskFocus::Buttons;
                }
                _ => {}
            },
            CreateTaskFocus::Buttons => match (key.code, key.modifiers) {
                (KeyCode::Left, _) | (KeyCode::Char('h'), _) => {
                    state.selected_button = 0;
                }
                (KeyCode::Right, _) | (KeyCode::Char('l'), _) => {
                    state.selected_button = 1;
                }
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

    if state.focus == CreateTaskFocus::Description {
        let (inner_w, inner_h) = create_task_modal_desc_inner_dims(current_terminal_rect());
        state.description.ensure_cursor_visible(inner_w, inner_h);
    } else if state.focus == CreateTaskFocus::Title {
        let inner_w = create_task_modal_title_inner_w(current_terminal_rect());
        state.title.ensure_cursor_visible(inner_w.max(1), 1);
    }

    if close {
        return;
    }
    if submit {
        submit_create_task_state(app, state);
        return;
    }
    app.ui.create_task = Some(state);
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

    set_toast(
        app,
        "Creating task…".to_string(),
        Color::Cyan,
        Some(Instant::now() + Duration::from_secs(2)),
    );

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match create_task_http(&base_url, project_id, &title, description.as_deref(), status)
            .await
        {
            Ok(task_id) => {
                let _ = net_tx.send(NetEvent::TaskCreated { task_id, status }).await;
                let _ = net_tx
                    .send(NetEvent::Notice(format!("Created task: {title}")))
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("create task failed: {e}")))
                    .await;
            }
        }
    });
}

pub(crate) fn render_create_task_modal(f: &mut Frame, app: &AppState, state: &CreateTaskState) {
    let area = centered_rect(75, 70, f.area());
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
        Constraint::Length(7), // description
        Constraint::Length(3), // status
        Constraint::Length(3), // buttons
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
