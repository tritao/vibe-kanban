use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect, widgets::Clear};

use super::UiComponent;
use crate::state::{AppState, DiffFocus, FocusPane};

mod hit_test;
mod nav;
mod render;

#[derive(Debug, Clone, Copy)]
pub(super) struct CommitFooter {
    pub(super) loading: bool,
    pub(super) has_more: bool,
}

pub(super) fn commit_footer(app: &AppState) -> CommitFooter {
    let Some(repo_id) = crate::state::repo_scope::selected_repo_id(app) else {
        return CommitFooter {
            loading: false,
            has_more: false,
        };
    };
    let loading = app
        .diff
        .commits_by_repo
        .get(&repo_id)
        .map(|s| s.loading.visible())
        .unwrap_or(false);
    let has_more = app
        .diff
        .commits_by_repo
        .get(&repo_id)
        .map(|s| s.has_more)
        .unwrap_or(false);
    CommitFooter { loading, has_more }
}

pub(super) enum DiffListEvent {
    Key(KeyEvent),
    ClickIndex(usize),
    WheelDelta(i32),
}

pub(super) struct DiffList;

impl DiffList {
    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        f.render_widget(Clear, area);
        match app.diff.list_mode {
            crate::state::DiffListMode::Commits => render::render_commit_list(f, app, area),
            crate::state::DiffListMode::Files => render::render_files_list(f, app, area),
        }
    }

    fn on_event(app: &mut AppState, event: DiffListEvent) -> bool {
        if app.ui.focus != FocusPane::Diff || app.ui.diff_focus != DiffFocus::Files {
            return false;
        }

        use crossterm::event::KeyCode;
        match event {
            DiffListEvent::ClickIndex(idx) => match app.diff.list_mode {
                crate::state::DiffListMode::Files => nav::select_diff_file(app, idx),
                crate::state::DiffListMode::Commits => nav::select_commit(app, idx),
            },
            DiffListEvent::WheelDelta(delta) => {
                let delta = delta.clamp(-50, 50);
                if delta == 0 {
                    return false;
                }
                match app.diff.list_mode {
                    crate::state::DiffListMode::Files => nav::select_adjacent_diff_file(app, delta),
                    crate::state::DiffListMode::Commits => nav::select_adjacent_commit(app, delta),
                }
            }
            DiffListEvent::Key(key) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => match app.diff.list_mode {
                    crate::state::DiffListMode::Files => nav::select_adjacent_diff_file(app, -1),
                    crate::state::DiffListMode::Commits => nav::select_adjacent_commit(app, -1),
                },
                KeyCode::Down | KeyCode::Char('j') => match app.diff.list_mode {
                    crate::state::DiffListMode::Files => nav::select_adjacent_diff_file(app, 1),
                    crate::state::DiffListMode::Commits => nav::select_adjacent_commit(app, 1),
                },
                KeyCode::PageDown if app.diff.list_mode == crate::state::DiffListMode::Commits => {
                    crate::commands::request_commit_list_more(app);
                    true
                }
                _ => false,
            },
        }
    }
}

impl UiComponent for DiffList {
    type Event = DiffListEvent;

    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        DiffList::render(f, app, area);
    }

    fn hit_test(app: &AppState, area: Rect, _col: u16, row: u16) -> Option<Self::Event> {
        let idx = match app.diff.list_mode {
            crate::state::DiffListMode::Files => hit_test::diff_file_index(app, area, row),
            crate::state::DiffListMode::Commits => hit_test::commit_index(app, area, row),
        }?;
        Some(DiffListEvent::ClickIndex(idx))
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        DiffList::on_event(app, event)
    }
}
