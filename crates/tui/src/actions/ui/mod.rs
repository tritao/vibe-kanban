use std::time::Instant;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
};
use ratatui::style::Color;

pub(super) use super::selection as sel;
use crate::{commands::copy_to_clipboard_osc52, events::UiEvent, state::AppState};

mod composer;
mod confirm;
mod copy;
mod copy_targets;
mod keys;
mod keys_global;
mod modals;
mod mouse;
mod slash;

#[derive(Debug)]
pub(super) enum Effect {
    CopyOsc52(String),
    SetMouseCapture(bool),
    Toast {
        message: String,
        color: Color,
        expires_at: Option<Instant>,
    },
}

#[derive(Debug, Default)]
pub(super) struct UiApplyResult {
    pub(super) changed: bool,
    pub(super) should_quit: bool,
    pub(super) effects: Vec<Effect>,
}

impl UiApplyResult {
    pub(super) fn none() -> Self {
        Self::default()
    }

    pub(super) fn changed(changed: bool) -> Self {
        Self {
            changed,
            ..Self::default()
        }
    }

    #[allow(dead_code)]
    pub(super) fn quit() -> Self {
        Self {
            should_quit: true,
            ..Self::default()
        }
    }

    pub(super) fn with_effects(mut self, effects: Vec<Effect>) -> Self {
        self.effects = effects;
        self
    }

    pub(super) fn with_effect(mut self, effect: Effect) -> Self {
        self.effects.push(effect);
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum CopyTarget {
    Execution,
    DiffFiles,
    DiffPreview,
    WorktreePath,
    AttemptCheckoutPath,
}

pub(super) fn reduce_ui(app: &mut AppState, evt: UiEvent) -> anyhow::Result<UiApplyResult> {
    match evt {
        UiEvent::Tick => Ok(UiApplyResult::none()),
        UiEvent::Crossterm(Event::Key(key)) => {
            if key.kind != KeyEventKind::Press {
                return Ok(UiApplyResult::none());
            }
            Ok(keys::reduce_key(app, key))
        }
        UiEvent::Crossterm(Event::Mouse(mouse)) => {
            if !app.ui.mouse_capture_enabled {
                return Ok(UiApplyResult::none());
            }
            Ok(UiApplyResult::changed(mouse::reduce_mouse(app, mouse)))
        }
        _ => Ok(UiApplyResult::none()),
    }
}

pub(super) fn run_effects(app: &mut AppState, effects: Vec<Effect>) -> bool {
    let mut dirty = false;
    for eff in effects {
        match eff {
            Effect::CopyOsc52(text) => {
                if let Err(e) = copy_to_clipboard_osc52(&text) {
                    crate::ui::toasts::err_short(app, format!("Copy failed: {e}"));
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
                        crate::ui::toasts::ok_short(
                            app,
                            if enabled {
                                "Mouse capture enabled".to_string()
                            } else {
                                "Mouse capture disabled (terminal text selection enabled)"
                                    .to_string()
                            },
                        );
                        dirty = true;
                    }
                    Err(e) => {
                        crate::ui::toasts::err_short(
                            app,
                            format!("Mouse capture toggle failed: {e}"),
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
                let expires_in = expires_at.and_then(|t| t.checked_duration_since(Instant::now()));
                app.ui.set_toast(message, color, expires_in);
                dirty = true;
            }
        }
    }
    dirty
}

#[cfg(test)]
mod tests;
