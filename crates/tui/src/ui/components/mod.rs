mod board_pane;
mod diff_list;
mod diff_pane;
mod diff_preview;
mod diff_repo_bar;
mod exec_input;
mod exec_log;
mod exec_pane;
mod task_lines;

pub(crate) use board_pane::{BoardHit, BoardPane, BoardPaneEvent};
pub(crate) use diff_pane::{DiffPane, DiffPaneEvent, sync_selected_repo_from_diff_selection};
pub(crate) use diff_repo_bar::{DiffRepoAction, trigger_diff_repo_action};
pub(crate) use exec_pane::{ExecPane, ExecPaneEvent};
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
