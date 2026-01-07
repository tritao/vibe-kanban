pub(crate) mod diff_list;
pub(crate) mod diff_preview;
pub(crate) mod diff_repo_bar;
pub(crate) mod exec_input;
pub(crate) mod exec_log;

use ratatui::{Frame, layout::Rect};

use crate::state::AppState;

pub(crate) trait UiComponent {
    type Event;

    fn render(f: &mut Frame, app: &AppState, area: Rect);
    fn hit_test(app: &AppState, area: Rect, col: u16, row: u16) -> Option<Self::Event>;
    fn on_event(app: &mut AppState, event: Self::Event) -> bool;
}
