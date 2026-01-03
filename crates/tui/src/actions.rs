use std::time::Instant;

use ratatui::text::Line;
use ratatui::layout::Rect;

use crate::commands::{copy_to_clipboard_osc52, set_toast, update_git_activity_indicators};
use crate::controller::handle_net_event;
use crate::diff_preview::{
    diff_preview_refresh_ready, request_diff_preview_async, schedule_diff_preview_refresh,
};
use crate::events::{NetEvent, UiEvent};
use crate::layout::{clamp_scroll_offsets, compute_main_layout};
use crate::logs::{flush_log_buffers, mark_all_log_buffers_dirty};
use crate::state::AppState;

pub(crate) enum Action {
    Ui(UiEvent),
    Net(NetEvent),
    Tick { now: Instant, term: Rect },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CopyTarget {
    Execution,
    DiffFiles,
    DiffPreview,
}

pub(crate) enum Effect {
    CopyOsc52(String),
    Toast {
        message: String,
        color: ratatui::style::Color,
        expires_at: Option<Instant>,
    },
}

pub(crate) struct DispatchOutcome {
    pub(crate) quit: bool,
    pub(crate) dirty: bool,
}

pub(crate) fn dispatch(app: &mut AppState, action: Action) -> anyhow::Result<DispatchOutcome> {
    match action {
        Action::Ui(evt) => {
            let (quit, dirty, effects) = reduce_ui(app, evt)?;
            let effects_dirty = run_effects(app, effects);
            Ok(DispatchOutcome {
                quit,
                dirty: dirty || effects_dirty,
            })
        }
        Action::Net(evt) => {
            handle_net_event(app, evt);
            Ok(DispatchOutcome { quit: false, dirty: true })
        }
        Action::Tick { now, term } => Ok(DispatchOutcome {
            quit: false,
            dirty: reduce_tick(app, now, term),
        }),
    }
}

fn reduce_ui(app: &mut AppState, evt: UiEvent) -> anyhow::Result<(bool, bool, Vec<Effect>)> {
    use crossterm::event::{Event, KeyCode, KeyEventKind};

    // Intercept a few global actions here first; everything else is still handled by the legacy
    // input handler.
    if let UiEvent::Crossterm(Event::Key(key)) = &evt {
        // Only act on key press, not repeats/releases.
        if key.kind == KeyEventKind::Press {
            // If we're in a text editor, handle core edit/navigation here so state-mutation is
            // centralized in the dispatcher. (We still let the legacy handler manage open/close
            // and non-editor actions for now.)
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
                        crate::state::FocusPane::Execution => CopyTarget::Execution,
                        crate::state::FocusPane::Diff => match app.ui.diff_focus {
                            crate::state::DiffFocus::Files => CopyTarget::DiffFiles,
                            crate::state::DiffFocus::Preview => CopyTarget::DiffPreview,
                        },
                        _ => return Ok((false, false, vec![])),
                    };
                    let effects = reduce_copy(app, target);
                    return Ok((false, false, effects));
                }
            }
        }
    }

    // For now, keep the existing input handler as our "legacy reducer".
    let quit = crate::input::handle_ui_event(app, evt)?;
    Ok((quit, !quit, vec![]))
}

fn reduce_composer_key(
    app: &mut AppState,
    key: crossterm::event::KeyEvent,
) -> Option<(bool, Vec<Effect>)> {
    use crossterm::event::{KeyCode, KeyModifiers};

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
            // Slash-mode autocomplete navigation if cursor is at the end.
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
    use crossterm::event::{KeyCode, KeyModifiers};

    // Let the legacy handler deal with close/submit.
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

fn reduce_copy(app: &mut AppState, target: CopyTarget) -> Vec<Effect> {
    use ratatui::style::Color;

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

fn run_effects(app: &mut AppState, effects: Vec<Effect>) -> bool {
    let mut dirty = false;
    for eff in effects {
        match eff {
            Effect::CopyOsc52(text) => {
                if let Err(e) = copy_to_clipboard_osc52(&text) {
                    set_toast(
                        app,
                        format!("Copy failed: {e}"),
                        ratatui::style::Color::Red,
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

fn reduce_tick(app: &mut AppState, now: Instant, term: Rect) -> bool {
    let mut dirty = false;

    let layout = compute_main_layout(term);
    let inner_width = layout.exec_logs.width.saturating_sub(2);
    let width = inner_width as usize;
    if app.exec.log_render_width != inner_width {
        app.exec.log_render_width = inner_width;
        mark_all_log_buffers_dirty(app, 0);
    }
    if flush_log_buffers(app, width) {
        dirty = true;
    }

    // Throttle expensive diff preview rebuilds (highlighting/wrapping) during WS replay bursts by
    // only refreshing when explicitly requested.
    let diff_inner_width_u16 = layout.diff_preview.width.saturating_sub(2);
    let diff_inner_width = diff_inner_width_u16 as usize;
    let diff_width_changed = app.diff.diff_preview_cache_width != diff_inner_width_u16;
    let has_diffs = app
        .diff
        .diff_store
        .get("entries")
        .and_then(|v| v.as_object())
        .is_some_and(|o| !o.is_empty());
    if diff_width_changed {
        app.diff.diff_preview_cache_width = diff_inner_width_u16;
        app.diff.diff_preview_cache_key = None;
        if has_diffs {
            schedule_diff_preview_refresh(app, std::time::Duration::from_millis(0));
        }
    }
    if app.diff.diff_preview_cache_key.is_none()
        && !app.diff.diff_preview_pending
        && app.diff.diff_preview_job.is_none()
        && has_diffs
    {
        schedule_diff_preview_refresh(app, std::time::Duration::from_millis(0));
    }
    if diff_preview_refresh_ready(app, now) && app.diff.diff_preview_job.is_none() {
        if has_diffs {
            request_diff_preview_async(app, diff_inner_width);
            dirty = true;
        } else if app.diff.diff_preview_lines != vec![Line::from("No diffs")] {
            app.diff.diff_preview_lines = vec![Line::from("No diffs")];
            dirty = true;
        }
        app.diff.diff_preview_pending = false;
        app.diff.diff_preview_next_refresh_at = None;
    }

    if clamp_scroll_offsets(app, layout) {
        dirty = true;
    }
    if update_git_activity_indicators(app, now) {
        dirty = true;
    }

    dirty
}
