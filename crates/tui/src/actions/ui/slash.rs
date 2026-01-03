use crate::state::{AppState};

pub(super) fn is_slash_mode(app: &AppState) -> bool {
    crate::slash::composer_is_slash_mode(&app.ui.composer.buffer)
}

pub(super) fn move_autocomplete(app: &mut AppState, delta: i32) {
    crate::slash::move_composer_autocomplete(app, delta);
}

pub(super) fn apply_autocomplete(app: &mut AppState) -> bool {
    crate::slash::apply_composer_autocomplete(app)
}

