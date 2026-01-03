use std::time::Instant;

use crossterm::event::{Event, KeyEventKind};
use ratatui::style::Color;

use crate::commands::{copy_to_clipboard_osc52, set_toast};
use crate::events::UiEvent;
use crate::state::AppState;

pub(super) use super::selection as sel;

mod copy;
mod focus;
mod keys;
mod modals;
mod mouse;
mod scroll;

pub(super) enum Effect {
    CopyOsc52(String),
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
        UiEvent::Crossterm(Event::Mouse(mouse)) => Ok((false, mouse::reduce_mouse(app, mouse), vec![])),
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
