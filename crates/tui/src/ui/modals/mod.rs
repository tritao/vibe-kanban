use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::Frame;

use crate::state::AppState;

mod branch_picker;
mod confirm;
mod help;
mod input;
mod project_setup;

pub(crate) use branch_picker::render_branch_picker_modal;
pub(crate) use confirm::render_confirm_modal;
pub(crate) use help::render_help_modal;
pub(crate) use input::render_input_modal;
pub(crate) use project_setup::render_project_setup_modal;

pub(crate) fn open_help(app: &mut AppState) {
    app.ui.show_help = true;
}

pub(crate) fn open_search(app: &mut AppState) {
    input::open_search(app);
}

pub(crate) fn modal_blocks_mouse(app: &AppState) -> bool {
    app.ui.confirm.is_some()
        || app.ui.show_help
        || app.ui.create_task.is_some()
        || app.ui.project_setup.is_some()
        || app.ui.branch_picker.is_some()
        || app.ui.input.is_some()
}

pub(crate) fn handle_modal_mouse(app: &mut AppState, mouse: MouseEvent) -> bool {
    input::handle_search_caret_click(app, mouse)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ModalKeyResult {
    pub(crate) quit: bool,
    pub(crate) dirty: bool,
}

pub(crate) fn reduce_modal_key(app: &mut AppState, key: KeyEvent) -> Option<ModalKeyResult> {
    if app.ui.confirm.is_some() {
        let dirty = confirm::handle_confirm_key(app, key);
        return Some(ModalKeyResult { quit: false, dirty });
    }
    if app.ui.input.is_some() {
        let dirty = input::handle_search_key(app, key);
        return Some(ModalKeyResult { quit: false, dirty });
    }
    if app.ui.show_help {
        let dirty = help::handle_help_key(app, key);
        return Some(ModalKeyResult { quit: false, dirty });
    }
    if app.ui.branch_picker.is_some() {
        let dirty = branch_picker::handle_branch_picker_key(app, key);
        return Some(ModalKeyResult { quit: false, dirty });
    }
    if app.ui.project_setup.is_some() {
        let dirty = project_setup::handle_project_setup_key(app, key);
        return Some(ModalKeyResult { quit: false, dirty });
    }
    if app.ui.create_task.is_some() {
        let dirty = crate::ui::handle_create_task_key(app, key);
        return Some(ModalKeyResult { quit: false, dirty });
    }
    None
}

pub(crate) fn render_overlays(f: &mut Frame, app: &AppState) {
    if app.ui.show_help {
        render_help_modal(f);
    }
    if let Some(state) = app.ui.create_task.as_ref() {
        crate::ui::render_create_task_modal(f, app, state);
    }
    if let Some(confirm) = app.ui.confirm.as_ref() {
        render_confirm_modal(f, confirm);
    }
    if let Some(input) = app.ui.input.as_ref() {
        render_input_modal(f, input);
    }
    if let Some(state) = app.ui.project_setup.as_ref() {
        render_project_setup_modal(f, state);
    }
    if let Some(state) = app.ui.branch_picker.as_ref() {
        render_branch_picker_modal(f, state);
    }
}
