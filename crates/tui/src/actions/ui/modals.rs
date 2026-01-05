use super::sel;
use crate::{
    layout::current_terminal_rect,
    state::{AppState, ConfirmState, InputMode, InputState, TextFieldState},
};

pub(super) fn modal_blocks_mouse(app: &AppState) -> bool {
    app.ui.confirm.is_some() || app.ui.show_help || app.ui.create_task.is_some()
}

pub(super) fn open_help(app: &mut AppState) {
    app.ui.show_help = true;
}

pub(super) fn close_help(app: &mut AppState) {
    app.ui.show_help = false;
}

pub(super) fn open_confirm(app: &mut AppState, state: ConfirmState) {
    app.ui.confirm = Some(state);
}

pub(super) fn close_confirm(app: &mut AppState) {
    app.ui.confirm = None;
}

pub(super) fn open_search(app: &mut AppState) {
    let mut field = TextFieldState::default();
    field.buffer = app.board.task_filter.clone();
    field.set_end();
    let term = current_terminal_rect();
    let area = crate::ui::layout::centered_rect(80, 25, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1);
    field.ensure_cursor_visible(content_w, 1);
    app.ui.input = Some(InputState {
        mode: InputMode::SearchTasks,
        field,
        original: app.board.task_filter.clone(),
    });
}

pub(super) fn close_search(app: &mut AppState, restore_original: bool) {
    if let Some(input) = app.ui.input.take() {
        if restore_original {
            app.board.task_filter = input.original;
        }
    }
    sel::ensure_selection_visible(app);
}

pub(super) fn close_search_keep(app: &mut AppState) {
    app.ui.input = None;
    sel::ensure_selection_visible(app);
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
