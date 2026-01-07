use crate::state::{AppState, FocusPane};

pub(super) fn focus_board(app: &mut AppState) {
    app.ui.focus = FocusPane::Board;
}

pub(super) fn focus_execution(app: &mut AppState) {
    app.ui.focus = FocusPane::Execution;
}

pub(super) fn cycle_focus(app: &mut AppState) {
    app.ui.focus = match app.ui.focus {
        FocusPane::Board => FocusPane::Execution,
        FocusPane::Execution => FocusPane::Diff,
        FocusPane::Diff => FocusPane::Board,
    };
}
