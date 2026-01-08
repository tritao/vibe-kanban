mod board_pane;
mod diff_list;
mod diff_pane;
mod diff_preview;
mod diff_repo_bar;
mod exec_input;
mod exec_log;
mod exec_pane;
mod task_lines;

pub(crate) use board_pane::{BoardHit, BoardPane};
pub(crate) use diff_pane::{DiffPane, sync_selected_repo_from_diff_selection};
pub(crate) use diff_repo_bar::{DiffRepoAction, trigger_diff_repo_action};
pub(crate) use exec_pane::ExecPane;
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

pub(crate) fn handle_board_key(app: &mut AppState, key: crossterm::event::KeyEvent) -> bool {
    <BoardPane as UiComponent>::on_event(app, board_pane::BoardPaneEvent::Key(key))
}

pub(crate) fn handle_board_mouse(
    app: &mut AppState,
    mouse: crossterm::event::MouseEvent,
    area: Rect,
) -> bool {
    <BoardPane as UiComponent>::on_event(app, board_pane::BoardPaneEvent::Mouse { mouse, area })
}

pub(crate) fn handle_diff_key(app: &mut AppState, key: crossterm::event::KeyEvent) -> bool {
    <DiffPane as UiComponent>::on_event(app, diff_pane::DiffPaneEvent::Key(key))
}

pub(crate) fn handle_diff_mouse(
    app: &mut AppState,
    mouse: crossterm::event::MouseEvent,
    area: Rect,
) -> bool {
    <DiffPane as UiComponent>::on_event(app, diff_pane::DiffPaneEvent::Mouse { mouse, area })
}

pub(crate) fn handle_exec_key(app: &mut AppState, key: crossterm::event::KeyEvent) -> bool {
    <ExecPane as UiComponent>::on_event(app, exec_pane::ExecPaneEvent::Key(key))
}

pub(crate) fn handle_exec_mouse(
    app: &mut AppState,
    mouse: crossterm::event::MouseEvent,
    area: Rect,
) -> bool {
    <ExecPane as UiComponent>::on_event(app, exec_pane::ExecPaneEvent::Mouse { mouse, area })
}
