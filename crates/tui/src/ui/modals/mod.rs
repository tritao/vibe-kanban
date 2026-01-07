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

trait ModalComponent: Sync {
    fn is_open(&self, app: &AppState) -> bool;
    fn blocks_mouse(&self, app: &AppState) -> bool {
        self.is_open(app)
    }
    fn render(&self, f: &mut Frame, app: &AppState);
    fn on_key(&self, app: &mut AppState, key: KeyEvent) -> bool;
    fn on_mouse(&self, _app: &mut AppState, _mouse: MouseEvent) -> bool {
        false
    }
}

struct ConfirmModal;
impl ModalComponent for ConfirmModal {
    fn is_open(&self, app: &AppState) -> bool {
        app.ui.confirm.is_some()
    }
    fn render(&self, f: &mut Frame, app: &AppState) {
        if let Some(confirm) = app.ui.confirm.as_ref() {
            render_confirm_modal(f, confirm);
        }
    }
    fn on_key(&self, app: &mut AppState, key: KeyEvent) -> bool {
        confirm::handle_confirm_key(app, key)
    }
}

struct InputModal;
impl ModalComponent for InputModal {
    fn is_open(&self, app: &AppState) -> bool {
        app.ui.input.is_some()
    }
    fn render(&self, f: &mut Frame, app: &AppState) {
        if let Some(input) = app.ui.input.as_ref() {
            render_input_modal(f, input);
        }
    }
    fn on_key(&self, app: &mut AppState, key: KeyEvent) -> bool {
        input::handle_search_key(app, key)
    }
    fn on_mouse(&self, app: &mut AppState, mouse: MouseEvent) -> bool {
        input::handle_search_caret_click(app, mouse)
    }
}

struct HelpModal;
impl ModalComponent for HelpModal {
    fn is_open(&self, app: &AppState) -> bool {
        app.ui.show_help
    }
    fn render(&self, f: &mut Frame, _app: &AppState) {
        render_help_modal(f);
    }
    fn on_key(&self, app: &mut AppState, key: KeyEvent) -> bool {
        help::handle_help_key(app, key)
    }
}

struct BranchPickerModal;
impl ModalComponent for BranchPickerModal {
    fn is_open(&self, app: &AppState) -> bool {
        app.ui.branch_picker.is_some()
    }
    fn render(&self, f: &mut Frame, app: &AppState) {
        if let Some(state) = app.ui.branch_picker.as_ref() {
            render_branch_picker_modal(f, state);
        }
    }
    fn on_key(&self, app: &mut AppState, key: KeyEvent) -> bool {
        branch_picker::handle_branch_picker_key(app, key)
    }
}

struct ProjectSetupModal;
impl ModalComponent for ProjectSetupModal {
    fn is_open(&self, app: &AppState) -> bool {
        app.ui.project_setup.is_some()
    }
    fn render(&self, f: &mut Frame, app: &AppState) {
        if let Some(state) = app.ui.project_setup.as_ref() {
            render_project_setup_modal(f, state);
        }
    }
    fn on_key(&self, app: &mut AppState, key: KeyEvent) -> bool {
        project_setup::handle_project_setup_key(app, key)
    }
}

struct CreateTaskModal;
impl ModalComponent for CreateTaskModal {
    fn is_open(&self, app: &AppState) -> bool {
        app.ui.create_task.is_some()
    }
    fn render(&self, f: &mut Frame, app: &AppState) {
        if let Some(state) = app.ui.create_task.as_ref() {
            crate::ui::render_create_task_modal(f, app, state);
        }
    }
    fn on_key(&self, app: &mut AppState, key: KeyEvent) -> bool {
        crate::ui::handle_create_task_key(app, key)
    }
}

static CONFIRM_MODAL: ConfirmModal = ConfirmModal;
static INPUT_MODAL: InputModal = InputModal;
static HELP_MODAL: HelpModal = HelpModal;
static BRANCH_PICKER_MODAL: BranchPickerModal = BranchPickerModal;
static PROJECT_SETUP_MODAL: ProjectSetupModal = ProjectSetupModal;
static CREATE_TASK_MODAL: CreateTaskModal = CreateTaskModal;

// Key dispatch order (highest priority first).
static KEY_ORDER: [&dyn ModalComponent; 6] = [
    &CONFIRM_MODAL,
    &INPUT_MODAL,
    &HELP_MODAL,
    &BRANCH_PICKER_MODAL,
    &PROJECT_SETUP_MODAL,
    &CREATE_TASK_MODAL,
];

// Render order (matches previous behavior).
static RENDER_ORDER: [&dyn ModalComponent; 6] = [
    &HELP_MODAL,
    &CREATE_TASK_MODAL,
    &CONFIRM_MODAL,
    &INPUT_MODAL,
    &PROJECT_SETUP_MODAL,
    &BRANCH_PICKER_MODAL,
];

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
}
