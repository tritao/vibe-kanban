use crate::state::AppState;

pub(crate) fn info_short(app: &mut AppState, message: impl Into<String>) {
    crate::ui::guards::toast_short(app, message, crate::ui::palette::toast_info());
}

pub(crate) fn ok_short(app: &mut AppState, message: impl Into<String>) {
    crate::ui::guards::toast_short(app, message, crate::ui::palette::toast_ok());
}

pub(crate) fn warn_short(app: &mut AppState, message: impl Into<String>) {
    crate::ui::guards::toast_short(app, message, crate::ui::palette::toast_warn());
}

pub(crate) fn err_short(app: &mut AppState, message: impl Into<String>) {
    crate::ui::guards::toast_short(app, message, crate::ui::palette::toast_err());
}

pub(crate) fn ok_medium(app: &mut AppState, message: impl Into<String>) {
    crate::ui::guards::toast_medium(app, message, crate::ui::palette::toast_ok());
}

pub(crate) fn err_medium(app: &mut AppState, message: impl Into<String>) {
    crate::ui::guards::toast_medium(app, message, crate::ui::palette::toast_err());
}

pub(crate) fn ok_seconds(app: &mut AppState, message: impl Into<String>, seconds: u64) {
    crate::ui::guards::toast_seconds(app, message, crate::ui::palette::toast_ok(), seconds);
}

pub(crate) fn warn_seconds(app: &mut AppState, message: impl Into<String>, seconds: u64) {
    crate::ui::guards::toast_seconds(app, message, crate::ui::palette::toast_warn(), seconds);
}

pub(crate) fn warn_sticky(app: &mut AppState, message: impl Into<String>) {
    app.ui
        .set_toast(message, crate::ui::palette::toast_warn(), None);
}
