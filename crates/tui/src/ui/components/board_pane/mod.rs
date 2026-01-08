use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};

use super::UiComponent;
use crate::state::{AppState, TaskStatus};

mod hit_test;
mod input;
mod layout;
mod render;

pub(crate) use hit_test::board_hit_at;
pub(crate) use render::render_board_pane;

#[derive(Debug, Clone, Copy)]
pub(crate) struct BoardHit {
    pub(crate) status: TaskStatus,
    pub(crate) clicked_index: Option<usize>,
    pub(crate) clicked_task_id: Option<uuid::Uuid>,
}

pub(crate) enum BoardPaneEvent {
    Key(KeyEvent),
    Mouse { mouse: MouseEvent, area: Rect },
}

pub(crate) struct BoardPane;

impl UiComponent for BoardPane {
    type Event = BoardPaneEvent;

    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        render_board_pane(f, app, area);
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        input::handle_event(app, event)
    }
}
