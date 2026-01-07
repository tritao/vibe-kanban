use std::time::Instant;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
};
use ratatui::style::Color;

pub(super) use super::selection as sel;
use crate::{
    commands::{copy_to_clipboard_osc52, set_toast},
    events::UiEvent,
    state::AppState,
};

mod composer;
mod confirm;
mod copy;
mod focus;
mod keys;
mod keys_board;
mod keys_diff;
mod keys_exec;
mod keys_global;
mod modals;
mod mouse;
mod slash;

pub(super) enum Effect {
    CopyOsc52(String),
    SetMouseCapture(bool),
    Toast {
        message: String,
        color: Color,
        expires_at: Option<Instant>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) enum CopyTarget {
    Execution,
    DiffFiles,
    DiffPreview,
    WorktreePath,
    AttemptCheckoutPath,
}

pub(super) fn reduce_ui(
    app: &mut AppState,
    evt: UiEvent,
) -> anyhow::Result<(bool, bool, Vec<Effect>)> {
    match evt {
        UiEvent::Tick => Ok((false, false, vec![])),
        UiEvent::Crossterm(Event::Key(key)) => {
            if key.kind != KeyEventKind::Press {
                return Ok((false, false, vec![]));
            }
            Ok(keys::reduce_key(app, key))
        }
        UiEvent::Crossterm(Event::Mouse(mouse)) => {
            if !app.ui.mouse_capture_enabled {
                return Ok((false, false, vec![]));
            }
            Ok((false, mouse::reduce_mouse(app, mouse), vec![]))
        }
        _ => Ok((false, false, vec![])),
    }
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
            Effect::SetMouseCapture(enabled) => {
                let res = if enabled {
                    execute!(std::io::stdout(), EnableMouseCapture)
                } else {
                    execute!(std::io::stdout(), DisableMouseCapture)
                };
                match res {
                    Ok(()) => {
                        app.ui.mouse_capture_enabled = enabled;
                        set_toast(
                            app,
                            if enabled {
                                "Mouse capture enabled".to_string()
                            } else {
                                "Mouse capture disabled (terminal text selection enabled)"
                                    .to_string()
                            },
                            Color::Green,
                            Some(Instant::now() + std::time::Duration::from_secs(2)),
                        );
                        dirty = true;
                    }
                    Err(e) => {
                        set_toast(
                            app,
                            format!("Mouse capture toggle failed: {e}"),
                            Color::Red,
                            Some(Instant::now() + std::time::Duration::from_secs(2)),
                        );
                        dirty = true;
                    }
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

#[cfg(test)]
mod tests;
