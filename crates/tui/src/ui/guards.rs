use std::time::Duration;

use ratatui::style::Color;
use uuid::Uuid;

use crate::{commands::request_branch_status_refresh, state::AppState};

pub(crate) fn toast(
    app: &mut AppState,
    message: impl Into<String>,
    color: Color,
    duration: Duration,
) {
    app.ui.set_toast(message, color, Some(duration));
}

pub(crate) fn toast_short(app: &mut AppState, message: impl Into<String>, color: Color) {
    toast(app, message, color, crate::ui::constants::TOAST_SHORT);
}

pub(crate) fn toast_medium(app: &mut AppState, message: impl Into<String>, color: Color) {
    toast(app, message, color, crate::ui::constants::TOAST_MEDIUM);
}

pub(crate) fn toast_seconds(
    app: &mut AppState,
    message: impl Into<String>,
    color: Color,
    seconds: u64,
) {
    toast(app, message, color, Duration::from_secs(seconds));
}

pub(crate) fn ensure_attempt_selected(app: &mut AppState, context: &str) -> Option<Uuid> {
    let Some(attempt_id) = app.board.selected_attempt_id else {
        toast_short(
            app,
            format!("{context}: no attempt selected"),
            crate::ui::palette::toast_err(),
        );
        return None;
    };
    Some(attempt_id)
}

pub(crate) fn ensure_repo_status_loaded(app: &mut AppState, context: &str) -> bool {
    if !app.diff.repo_statuses.is_empty() {
        return true;
    }
    request_branch_status_refresh(app);
    toast_short(
        app,
        format!("{context}: loading repo status…"),
        crate::ui::palette::toast_warn(),
    );
    false
}
