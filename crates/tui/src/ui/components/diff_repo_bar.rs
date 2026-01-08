mod actions;
mod badges;
mod buttons;
mod hit_test;
mod layout;
mod render;
mod shared;

pub(crate) use actions::trigger_diff_repo_action;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use super::UiComponent;
use crate::state::AppState;

pub(crate) enum DiffRepoBarEvent {
    Key(KeyEvent),
    Action(DiffRepoAction),
}

pub(crate) struct DiffRepoBar;

#[derive(Debug, Clone, Copy)]
pub(crate) enum DiffRepoAction {
    Merge,
    CreatePr,
    OpenPr,
    Rebase,
    ResolveConflicts,
    OpenConflict,
    AbortConflicts,
    RefreshStatus,
}

impl DiffRepoAction {
    pub(super) fn git_op_kind(self) -> Option<crate::events::GitOpKind> {
        match self {
            DiffRepoAction::RefreshStatus => Some(crate::events::GitOpKind::Status),
            DiffRepoAction::Merge => Some(crate::events::GitOpKind::Merge),
            DiffRepoAction::Rebase => Some(crate::events::GitOpKind::Rebase),
            DiffRepoAction::CreatePr => Some(crate::events::GitOpKind::CreatePr),
            DiffRepoAction::AbortConflicts => Some(crate::events::GitOpKind::Abort),
            DiffRepoAction::ResolveConflicts
            | DiffRepoAction::OpenConflict
            | DiffRepoAction::OpenPr => None,
        }
    }
}

impl UiComponent for DiffRepoBar {
    type Event = DiffRepoBarEvent;

    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        render::render_diff_repo_bar(f, app, area);
    }

    fn hit_test(app: &AppState, area: Rect, col: u16, row: u16) -> Option<Self::Event> {
        hit_test::diff_repo_bar_action_at(app, area, col, row).map(DiffRepoBarEvent::Action)
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        match event {
            DiffRepoBarEvent::Action(action) => {
                trigger_diff_repo_action(app, action);
                true
            }
            DiffRepoBarEvent::Key(key) => {
                let Some(action) = buttons::action_for_key(key.code) else {
                    return false;
                };
                trigger_diff_repo_action(app, action);
                true
            }
        }
    }
}
