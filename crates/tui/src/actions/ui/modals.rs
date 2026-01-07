use crate::{
    layout::current_terminal_rect,
    state::{AppState, ConfirmState},
};

pub(super) fn open_confirm(app: &mut AppState, state: ConfirmState) {
    app.ui.confirm = Some(state);
}

pub(super) fn open_composer(app: &mut AppState) {
    app.ui.composer_active = true;
    app.ui.composer_suggest_index = 0;
    app.ui.composer.set_end();
    let layout = crate::layout::compute_main_layout(current_terminal_rect(), app.ui.focus);
    let area = layout.exec_input;
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2) as usize;
    let prefix_w = crate::text::display_width("  ");
    let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);
    app.ui
        .composer
        .ensure_cursor_visible(content_w, inner_h.max(1));
}

pub(super) fn close_composer(app: &mut AppState) {
    app.ui.composer_active = false;
    app.ui.composer.clear();
    app.ui.composer_suggest_index = 0;
    app.ui.refresh_branch_status_after_send = false;
}
