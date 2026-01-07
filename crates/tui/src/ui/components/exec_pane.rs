use crossterm::event::{KeyCode, KeyEvent};

use super::{
    UiComponent,
    exec_log::{ExecLog, ExecLogEvent},
};
use crate::state::{AppState, FocusPane};

pub(crate) enum ExecPaneEvent {
    Key(KeyEvent),
}

pub(crate) struct ExecPane;

impl UiComponent for ExecPane {
    type Event = ExecPaneEvent;

    fn render(f: &mut ratatui::Frame, app: &AppState, area: ratatui::layout::Rect) {
        crate::ui::render_execution_pane(f, app, area);
    }

    fn hit_test(
        _app: &AppState,
        _area: ratatui::layout::Rect,
        _col: u16,
        _row: u16,
    ) -> Option<Self::Event> {
        None
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        let ExecPaneEvent::Key(key) = event;

        if app.ui.focus != FocusPane::Execution {
            return false;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('i'), _) => {
                crate::ui::open_composer(app);
                true
            }
            _ => <ExecLog as UiComponent>::on_event(app, ExecLogEvent::Key(key)),
        }
    }
}
