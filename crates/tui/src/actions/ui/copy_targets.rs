use super::CopyTarget;
use crate::state::{AppState, DiffFocus, FocusPane};

pub(super) fn copy_target_for_focused_pane(app: &AppState) -> Option<CopyTarget> {
    match app.ui.focus {
        FocusPane::Execution => Some(CopyTarget::Execution),
        FocusPane::Diff => Some(match app.ui.diff_focus {
            DiffFocus::Files => CopyTarget::DiffFiles,
            DiffFocus::Preview => CopyTarget::DiffPreview,
        }),
        FocusPane::Board => None,
    }
}
