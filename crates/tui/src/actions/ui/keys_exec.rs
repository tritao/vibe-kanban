use crossterm::event::{KeyCode, KeyEvent};

use super::{Effect, modals};
use crate::{
    state::{AppState, FocusPane},
    ui::components::{
        UiComponent,
        exec_log::{ExecLog, ExecLogEvent},
    },
};

pub(super) fn handle_exec_key(app: &mut AppState, key: KeyEvent) -> Option<(bool, Vec<Effect>)> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('i'), _) if app.ui.focus == FocusPane::Execution => {
            modals::open_composer(app);
            return Some((true, vec![]));
        }
        _ if app.ui.focus == FocusPane::Execution => {
            if <ExecLog as UiComponent>::on_event(app, ExecLogEvent::Key(key)) {
                return Some((true, vec![]));
            }
        }
        _ => {}
    }
    None
}
