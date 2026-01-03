use std::time::Instant;

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::commands::{copy_to_clipboard_osc52, set_toast};
use crate::events::UiEvent;
use crate::layout::compute_main_layout;
use crate::state::{AppState, DiffFocus, FocusPane};

pub(super) enum Effect {
    CopyOsc52(String),
    Toast {
        message: String,
        color: Color,
        expires_at: Option<Instant>,
    },
}

#[derive(Debug, Clone, Copy)]
enum CopyTarget {
    Execution,
    DiffFiles,
    DiffPreview,
}

pub(super) fn reduce_ui(app: &mut AppState, evt: UiEvent) -> anyhow::Result<(bool, bool, Vec<Effect>)> {
    // Intercept a few global actions here first; everything else is still handled by the legacy
    // input handler.
    if let UiEvent::Crossterm(Event::Key(key)) = &evt {
        if key.kind == KeyEventKind::Press {
            if app.ui.create_task.is_some() {
                reduce_create_task_key(app, *key);
                return Ok((false, true, vec![]));
            }

            if app.ui.composer_active {
                if let Some((dirty, effects)) = reduce_composer_key(app, *key) {
                    return Ok((false, dirty, effects));
                }
            }

            if app.ui.input.is_some() {
                if let Some((dirty, effects)) = reduce_search_key(app, *key) {
                    return Ok((false, dirty, effects));
                }
            }

            if key.code == KeyCode::Char('y') {
                let blocked = app.ui.confirm.is_some()
                    || app.ui.input.is_some()
                    || app.ui.show_help
                    || app.ui.create_task.is_some()
                    || app.ui.composer_active;
                if !blocked {
                    let target = match app.ui.focus {
                        FocusPane::Execution => CopyTarget::Execution,
                        FocusPane::Diff => match app.ui.diff_focus {
                            DiffFocus::Files => CopyTarget::DiffFiles,
                            DiffFocus::Preview => CopyTarget::DiffPreview,
                        },
                        _ => return Ok((false, false, vec![])),
                    };
                    let effects = reduce_copy(app, target);
                    return Ok((false, false, effects));
                }
            }
        }
    }

    let quit = crate::input::handle_ui_event(app, evt)?;
    Ok((quit, !quit, vec![]))
}

pub(super) fn run_effects(app: &mut AppState, effects: Vec<Effect>) -> bool {
    let mut dirty = false;
    for eff in effects {
        match eff {
            Effect::CopyOsc52(text) => {
                if let Err(e) = copy_to_clipboard_osc52(&text) {
                    set_toast(
                        app,
                        format!("Copy failed: {e}"),
                        Color::Red,
                        Some(Instant::now() + std::time::Duration::from_secs(2)),
                    );
                    dirty = true;
                }
            }
            Effect::Toast {
                message,
                color,
                expires_at,
            } => {
                set_toast(app, message, color, expires_at);
                dirty = true;
            }
        }
    }
    dirty
}

fn reduce_copy(app: &mut AppState, target: CopyTarget) -> Vec<Effect> {
    let text = match target {
        CopyTarget::Execution => {
            if let Some(sel) = app.exec.log_selected {
                app.exec
                    .log_buffers
                    .get(&sel.exec_id)
                    .and_then(|b| b.rendered_entry_text(sel.entry_idx))
            } else {
                None
            }
            .unwrap_or_else(|| {
                let layout = compute_main_layout(crate::layout::current_terminal_rect());
                let area = layout.exec_logs;
                let len = app.exec.log_lines.len();
                let max_render = area.height.saturating_sub(2) as usize;
                let visible = max_render.min(len).max(1);
                let mut offset = if app.exec.log_autoscroll {
                    0
                } else {
                    app.exec.log_scroll_offset
                };
                offset = offset.min(len.saturating_sub(visible));
                let start = len.saturating_sub(visible + offset);
                let end = len.saturating_sub(offset);
                crate::util::lines_plain_text(app.exec.log_lines.get(start..end).unwrap_or(&[]))
            })
        }
        CopyTarget::DiffFiles => {
            let rows = crate::diff::diff_rows_with_all(&app.diff.diff_store);
            rows.get(app.diff.selected_diff_index)
                .map(|d| d.key.clone())
                .unwrap_or_default()
        }
        CopyTarget::DiffPreview => crate::util::lines_plain_text(&app.diff.diff_preview_lines),
    };

    if text.trim().is_empty() {
        return vec![Effect::Toast {
            message: "Copy: nothing to copy".to_string(),
            color: Color::Yellow,
            expires_at: Some(Instant::now() + std::time::Duration::from_secs(2)),
        }];
    }

    let label = match target {
        CopyTarget::Execution => "Copied logs",
        CopyTarget::DiffFiles => "Copied path",
        CopyTarget::DiffPreview => "Copied diff",
    };

    vec![
        Effect::CopyOsc52(text),
        Effect::Toast {
            message: label.to_string(),
            color: Color::Green,
            expires_at: Some(Instant::now() + std::time::Duration::from_secs(2)),
        },
    ]
}

fn reduce_composer_key(
    app: &mut AppState,
    key: crossterm::event::KeyEvent,
) -> Option<(bool, Vec<Effect>)> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            app.ui.composer.undo();
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL)
        | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
            app.ui.composer.redo();
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            app.ui.composer.clear();
        }
        (KeyCode::Enter, KeyModifiers::CONTROL) => {
            app.ui.composer.insert_char('\n');
        }
        (KeyCode::Left, KeyModifiers::CONTROL) => {
            app.ui.composer.move_word_left();
        }
        (KeyCode::Right, KeyModifiers::CONTROL) => {
            app.ui.composer.move_word_right();
        }
        (KeyCode::Left, _) => {
            app.ui.composer.move_left();
        }
        (KeyCode::Right, _) => {
            app.ui.composer.move_right();
        }
        (KeyCode::Up, _) => {
            if app.ui.composer.buffer.trim_start().starts_with('/')
                && {
                    app.ui.composer.clamp_cursor();
                    app.ui.composer.cursor == app.ui.composer.buffer.len()
                }
            {
                crate::ui::move_composer_autocomplete(app, -1);
            } else {
                app.ui.composer.move_up();
            }
        }
        (KeyCode::Down, _) => {
            if app.ui.composer.buffer.trim_start().starts_with('/')
                && {
                    app.ui.composer.clamp_cursor();
                    app.ui.composer.cursor == app.ui.composer.buffer.len()
                }
            {
                crate::ui::move_composer_autocomplete(app, 1);
            } else {
                app.ui.composer.move_down();
            }
        }
        (KeyCode::Home, _) => {
            app.ui.composer.move_home(true);
        }
        (KeyCode::End, _) => {
            app.ui.composer.move_end(true);
        }
        (KeyCode::Backspace, KeyModifiers::ALT) | (KeyCode::Backspace, KeyModifiers::CONTROL) => {
            app.ui.composer.backspace_word();
        }
        (KeyCode::Backspace, _) => {
            app.ui.composer.backspace();
        }
        (KeyCode::Delete, KeyModifiers::CONTROL) => {
            app.ui.composer.delete_word();
        }
        (KeyCode::Delete, _) => {
            app.ui.composer.delete();
        }
        (KeyCode::Tab, _) => {
            crate::ui::apply_composer_autocomplete(app);
        }
        (KeyCode::Char(c), KeyModifiers::NONE) => {
            app.ui.composer.insert_char(c);
        }
        _ => return None,
    }

    app.ui.composer_suggest_index = 0;
    let layout = compute_main_layout(crate::layout::current_terminal_rect());
    let area = layout.exec_input;
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2) as usize;
    let prefix_w = crate::text::display_width("  ");
    let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);
    app.ui.composer.ensure_cursor_visible(content_w, inner_h.max(1));
    Some((true, vec![]))
}

fn reduce_search_key(
    app: &mut AppState,
    key: crossterm::event::KeyEvent,
) -> Option<(bool, Vec<Effect>)> {
    if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
        return None;
    }

    let Some(mut input) = app.ui.input.take() else {
        return None;
    };

    match (key.code, key.modifiers) {
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            input.field.undo();
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL)
        | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
            input.field.redo();
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            input.field.clear();
        }
        (KeyCode::Left, KeyModifiers::CONTROL) => {
            input.field.move_word_left();
        }
        (KeyCode::Right, KeyModifiers::CONTROL) => {
            input.field.move_word_right();
        }
        (KeyCode::Left, _) => {
            input.field.move_left();
        }
        (KeyCode::Right, _) => {
            input.field.move_right();
        }
        (KeyCode::Home, _) => {
            input.field.move_home(false);
        }
        (KeyCode::End, _) => {
            input.field.move_end(false);
        }
        (KeyCode::Backspace, KeyModifiers::ALT) | (KeyCode::Backspace, KeyModifiers::CONTROL) => {
            input.field.backspace_word();
        }
        (KeyCode::Backspace, _) => {
            input.field.backspace();
        }
        (KeyCode::Delete, KeyModifiers::CONTROL) => {
            input.field.delete_word();
        }
        (KeyCode::Delete, _) => {
            input.field.delete();
        }
        (KeyCode::Char(c), KeyModifiers::NONE) => {
            input.field.insert_char(c);
        }
        _ => return None,
    }

    app.board.task_filter = input.field.buffer.clone();
    let term = crate::layout::current_terminal_rect();
    let area = crate::ui::layout::centered_rect(80, 25, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1);
    input.field.ensure_cursor_visible(content_w, 1);
    crate::selection::ensure_selection_visible(app);

    app.ui.input = Some(input);
    Some((true, vec![]))
}

fn reduce_create_task_key(app: &mut AppState, key: crossterm::event::KeyEvent) {
    use crate::state::{CreateTaskFocus, TaskStatus};

    let Some(mut state) = app.ui.create_task.take() else {
        return;
    };
    state.error = None;

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
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::widgets::{Block, Borders};
        let area = crate::ui::layout::centered_rect(75, 70, term);
        let inner = Block::default().borders(Borders::ALL).inner(area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(7),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(inner);
        let desc = chunks[1];
        let inner_w = desc.width.saturating_sub(2) as usize;
        let inner_h = desc.height.saturating_sub(2) as usize;
        (inner_w.max(1), inner_h.max(1))
    }

    fn create_task_modal_title_inner_w(term: Rect) -> usize {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::widgets::{Block, Borders};
        let area = crate::ui::layout::centered_rect(75, 70, term);
        let inner = Block::default().borders(Borders::ALL).inner(area);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(7),
                Constraint::Length(3),
                Constraint::Length(3),
            ])
            .split(inner);
        let title = chunks[0];
        title.width.saturating_sub(2) as usize
    }

    let can_create = !state.title.buffer.trim().is_empty() && app.board.selected_project_id.is_some();
    let mut close = false;
    let mut submit = false;

    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => close = true,
        (KeyCode::Tab, KeyModifiers::SHIFT) => state.focus = prev_focus(state.focus),
        (KeyCode::Tab, _) => state.focus = next_focus(state.focus),
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
                (KeyCode::Enter, _) => state.focus = CreateTaskFocus::Description,
                (KeyCode::Char('z'), KeyModifiers::CONTROL) => { state.title.undo(); }
                (KeyCode::Char('y'), KeyModifiers::CONTROL)
                | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => { state.title.redo(); }
                (KeyCode::Left, KeyModifiers::CONTROL) => state.title.move_word_left(),
                (KeyCode::Right, KeyModifiers::CONTROL) => state.title.move_word_right(),
                (KeyCode::Left, _) => state.title.move_left(),
                (KeyCode::Right, _) => state.title.move_right(),
                (KeyCode::Home, _) => state.title.move_home(false),
                (KeyCode::End, _) => state.title.move_end(false),
                (KeyCode::Backspace, KeyModifiers::ALT)
                | (KeyCode::Backspace, KeyModifiers::CONTROL) => state.title.backspace_word(),
                (KeyCode::Backspace, _) => state.title.backspace(),
                (KeyCode::Delete, KeyModifiers::CONTROL) => state.title.delete_word(),
                (KeyCode::Delete, _) => state.title.delete(),
                (KeyCode::Char('u'), KeyModifiers::CONTROL) => state.title.clear(),
                (KeyCode::Char(c), KeyModifiers::NONE) => state.title.insert_char(c),
                _ => {}
            },
            CreateTaskFocus::Description => match (key.code, key.modifiers) {
                (KeyCode::Char('z'), KeyModifiers::CONTROL) => { state.description.undo(); }
                (KeyCode::Char('y'), KeyModifiers::CONTROL)
                | (KeyCode::Char('Z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT) => { state.description.redo(); }
                (KeyCode::Left, KeyModifiers::CONTROL) => state.description.move_word_left(),
                (KeyCode::Right, KeyModifiers::CONTROL) => state.description.move_word_right(),
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
                (KeyCode::Left, _) | (KeyCode::Char('h'), _) => state.status = status_cycle(state.status, -1),
                (KeyCode::Right, _) | (KeyCode::Char('l'), _) => state.status = status_cycle(state.status, 1),
                (KeyCode::Up, _) | (KeyCode::Char('k'), _) => state.status = status_cycle(state.status, -1),
                (KeyCode::Down, _) | (KeyCode::Char('j'), _) => state.status = status_cycle(state.status, 1),
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

    if state.focus == CreateTaskFocus::Description {
        let (inner_w, inner_h) = create_task_modal_desc_inner_dims(crate::layout::current_terminal_rect());
        state.description.ensure_cursor_visible(inner_w, inner_h);
    } else if state.focus == CreateTaskFocus::Title {
        let inner_w = create_task_modal_title_inner_w(crate::layout::current_terminal_rect());
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

fn submit_create_task_state(app: &mut AppState, state: crate::state::CreateTaskState) {
    let Some(project_id) = app.board.selected_project_id else {
        app.ui.create_task = Some(crate::state::CreateTaskState {
            error: Some("No project selected.".to_string()),
            ..state
        });
        return;
    };
    let title = state.title.buffer.trim().to_string();
    if title.is_empty() {
        app.ui.create_task = Some(crate::state::CreateTaskState {
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
        Some(Instant::now() + std::time::Duration::from_secs(2)),
    );

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match crate::net::ops::create_task_http(&base_url, project_id, &title, description.as_deref(), status)
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
