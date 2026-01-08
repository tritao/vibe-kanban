use std::time::Duration;

use ratatui::style::Color;

use crate::state::AppState;

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
