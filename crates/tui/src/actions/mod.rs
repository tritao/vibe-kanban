mod net;
mod selection;
mod tick;
mod ui;

use std::time::Instant;

use ratatui::layout::Rect;

use crate::{
    events::{NetEvent, UiEvent},
    state::AppState,
};

fn trace_actions_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("VIBE_TUI_TRACE_ACTIONS")
            .ok()
            .is_some_and(|v| {
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
    })
}

pub(crate) enum Action {
    Ui(UiEvent),
    Net(NetEvent),
    Tick { now: Instant, term: Rect },
}

pub(crate) struct DispatchOutcome {
    pub(crate) quit: bool,
    pub(crate) dirty: bool,
}

pub(crate) fn dispatch(app: &mut AppState, action: Action) -> anyhow::Result<DispatchOutcome> {
    if trace_actions_enabled() {
        match &action {
            Action::Ui(UiEvent::Crossterm(ev)) => {
                tracing::debug!(event=?ev, "Action::Ui");
            }
            Action::Ui(UiEvent::Tick) => {
                // omit spam
            }
            Action::Net(ev) => {
                tracing::debug!(event=?ev, "Action::Net");
            }
            Action::Tick { .. } => {
                // omit spam
            }
        }
    }
    match action {
        Action::Ui(evt) => {
            let (quit, dirty, effects) = ui::reduce_ui(app, evt)?;
            let effects_dirty = ui::run_effects(app, effects);
            Ok(DispatchOutcome {
                quit,
                dirty: dirty || effects_dirty,
            })
        }
        Action::Net(evt) => Ok(DispatchOutcome {
            quit: false,
            dirty: net::reduce_net_event(app, evt),
        }),
        Action::Tick { now, term } => Ok(DispatchOutcome {
            quit: false,
            dirty: tick::reduce_tick(app, now, term),
        }),
    }
}
