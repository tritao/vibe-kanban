mod tick;
mod ui;

use std::time::Instant;

use ratatui::layout::Rect;

use crate::controller::handle_net_event;
use crate::events::{NetEvent, UiEvent};
use crate::state::AppState;

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
    match action {
        Action::Ui(evt) => {
            let (quit, dirty, effects) = ui::reduce_ui(app, evt)?;
            let effects_dirty = ui::run_effects(app, effects);
            Ok(DispatchOutcome {
                quit,
                dirty: dirty || effects_dirty,
            })
        }
        Action::Net(evt) => {
            handle_net_event(app, evt);
            Ok(DispatchOutcome {
                quit: false,
                dirty: true,
            })
        }
        Action::Tick { now, term } => Ok(DispatchOutcome {
            quit: false,
            dirty: tick::reduce_tick(app, now, term),
        }),
    }
}

