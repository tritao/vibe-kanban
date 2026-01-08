pub(crate) mod board_pane;
mod diff_list;
pub(crate) mod diff_pane;
mod diff_preview;
pub(crate) mod diff_repo_bar;
mod exec_input;
mod exec_log;
pub(crate) mod exec_pane;
mod task_lines;

use ratatui::{Frame, layout::Rect};

use crate::state::AppState;

pub(crate) trait UiComponent {
    type Event;

    fn render(f: &mut Frame, app: &AppState, area: Rect);
    fn hit_test(_app: &AppState, _area: Rect, _col: u16, _row: u16) -> Option<Self::Event> {
        None
    }
    fn on_event(app: &mut AppState, event: Self::Event) -> bool;
}
