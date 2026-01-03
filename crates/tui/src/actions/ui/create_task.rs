use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::layout::current_terminal_rect;
use crate::state::{AppState, CreateTaskFocus, CreateTaskState, TaskStatus};

pub(super) fn handle_create_task_key(app: &mut AppState, key: KeyEvent) {
    let Some(mut state) = app.ui.create_task.take() else {
        return;
    };
    state.error = None;

    let mut close = false;
    let mut submit = false;

    let can_create = app.board.selected_project_id.is_some() && !state.title.buffer.trim().is_empty();

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
            CreateTaskFocus::Title => match (key.code, key.modifiers) {
                (KeyCode::Backspace, KeyModifiers::ALT)
                | (KeyCode::Backspace, KeyModifiers::CONTROL) => state.title.backspace_word(),
                (KeyCode::Delete, KeyModifiers::CONTROL) => state.title.delete_word(),
                (KeyCode::Backspace, _) => state.title.backspace(),
                (KeyCode::Delete, _) => state.title.delete(),
                (KeyCode::Left, _) => state.title.move_left(),
                (KeyCode::Right, _) => state.title.move_right(),
                (KeyCode::Home, _) => state.title.move_home(true),
                (KeyCode::End, _) => state.title.move_end(true),
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => state.title.clear(),
                (KeyCode::Enter, _) => state.focus = CreateTaskFocus::Description,
                (KeyCode::Char(c), KeyModifiers::NONE) => state.title.insert_char(c),
                _ => {}
            },
            CreateTaskFocus::Description => match (key.code, key.modifiers) {
                (KeyCode::Backspace, KeyModifiers::ALT)
                | (KeyCode::Backspace, KeyModifiers::CONTROL) => state.description.backspace_word(),
                (KeyCode::Delete, KeyModifiers::CONTROL) => state.description.delete_word(),
                (KeyCode::Backspace, _) => state.description.backspace(),
                (KeyCode::Delete, _) => state.description.delete(),
                (KeyCode::Left, _) => state.description.move_left(),
                (KeyCode::Right, _) => state.description.move_right(),
                (KeyCode::Up, _) => state.description.move_up(),
                (KeyCode::Down, _) => state.description.move_down(),
                (KeyCode::Home, _) => state.description.move_home(true),
                (KeyCode::End, _) => state.description.move_end(true),
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => state.description.clear(),
                (KeyCode::Enter, _) => state.description.insert_char('\n'),
                (KeyCode::Char(c), KeyModifiers::NONE) => state.description.insert_char(c),
                _ => {}
            },
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
        return;
    }

    if submit {
        submit_create_task_state(app, state);
        return;
    }

    app.ui.create_task = Some(state);
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

fn create_task_modal_desc_inner_dims(term: Rect) -> (usize, usize) {
    let area = crate::ui::layout::centered_rect(80, 25, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2) as usize;
    // Layout in render_create_task_modal:
    // - Title field: 3
    // - Description: 8
    // - Status: 3
    // - Buttons: 3
    // Total: 17 (+2 borders) within modal content.
    let desc_inner_w = inner_w.saturating_sub(2).max(1);
    let desc_inner_h = (inner_h
        .saturating_sub(3)
        .saturating_sub(3)
        .saturating_sub(3))
    .saturating_sub(2)
    .max(1);
    (desc_inner_w, desc_inner_h)
}

fn create_task_modal_title_inner_w(term: Rect) -> usize {
    let area = crate::ui::layout::centered_rect(80, 25, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    inner_w.saturating_sub(2).max(1)
}

fn ensure_cursor_visible(state: &mut CreateTaskState) {
    if state.focus == CreateTaskFocus::Description {
        let (inner_w, inner_h) = create_task_modal_desc_inner_dims(current_terminal_rect());
        state.description.ensure_cursor_visible(inner_w, inner_h);
    } else if state.focus == CreateTaskFocus::Title {
        let inner_w = create_task_modal_title_inner_w(current_terminal_rect());
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

    crate::commands::set_toast(
        app,
        "Creating task…".to_string(),
        Color::Cyan,
        Some(Instant::now() + std::time::Duration::from_secs(2)),
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
        )
        .await
        {
            Ok(task_id) => {
                let _ = net_tx
                    .send(crate::events::NetEvent::TaskCreated { task_id, status })
                    .await;
                let _ = net_tx
                    .send(crate::events::NetEvent::Notice(format!("Created task: {title}")))
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(crate::events::NetEvent::Error(format!("create task failed: {e}")))
                    .await;
            }
        }
    });
}

