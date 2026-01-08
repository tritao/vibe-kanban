use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::Frame;

use crate::state::AppState;

mod branch_picker;
mod component;
mod confirm;
mod create_task;
mod debug_overlay;
mod help;
mod input;
mod project_setup;

use component::ModalComponent;
pub(crate) use create_task::open_create_task_modal;

// Key dispatch order (highest priority first).
static KEY_ORDER: [&dyn ModalComponent; 6] = [
    &confirm::MODAL,
    &input::MODAL,
    &help::MODAL,
    &branch_picker::MODAL,
    &project_setup::MODAL,
    &create_task::MODAL,
];

// Render order (matches previous behavior).
static RENDER_ORDER: [&dyn ModalComponent; 6] = [
    &help::MODAL,
    &create_task::MODAL,
    &confirm::MODAL,
    &input::MODAL,
    &project_setup::MODAL,
    &branch_picker::MODAL,
];

// Non-interactive overlays (render-only).
static OVERLAY_ORDER: [&dyn ModalComponent; 1] = [&debug_overlay::OVERLAY];

pub(crate) fn open_help(app: &mut AppState) {
    app.ui.show_help = true;
}

pub(crate) fn open_search(app: &mut AppState) {
    input::open_search(app);
}

pub(crate) fn open_branch_picker(
    app: &mut AppState,
    mode: crate::state::BranchPickerMode,
    repo_id: uuid::Uuid,
    repo_name: String,
) {
    branch_picker::open_branch_picker(app, mode, repo_id, repo_name);
}

pub(crate) fn modal_blocks_mouse(app: &AppState) -> bool {
    KEY_ORDER.iter().any(|m| m.blocks_mouse(app))
}

pub(crate) fn handle_modal_mouse(app: &mut AppState, mouse: MouseEvent) -> bool {
    for m in KEY_ORDER {
        if m.is_open(app) && m.on_mouse(app, mouse) {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ModalKeyResult {
    pub(crate) quit: bool,
    pub(crate) dirty: bool,
}

pub(crate) fn reduce_modal_key(app: &mut AppState, key: KeyEvent) -> Option<ModalKeyResult> {
    for m in KEY_ORDER {
        if m.is_open(app) {
            let dirty = m.on_key(app, key);
            return Some(ModalKeyResult { quit: false, dirty });
        }
    }
    None
}

pub(crate) fn render_overlays(f: &mut Frame, app: &AppState) {
    for m in RENDER_ORDER {
        if m.is_open(app) {
            m.render(f, app);
        }
    }
    for o in OVERLAY_ORDER {
        if o.is_open(app) {
            o.render(f, app);
        }
    }
}
