use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{GitBranchItem, TaskStatus, TextFieldState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputMode {
    SearchTasks,
}

#[derive(Debug, Clone)]
pub(crate) struct InputState {
    pub(crate) mode: InputMode,
    pub(crate) field: TextFieldState,
    pub(crate) original: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ConfirmAction {
    StopExec {
        exec_id: Uuid,
    },
    DeleteTask {
        task_id: Uuid,
        delete_mode: DeleteTaskMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeleteTaskMode {
    Promote,
    Subtree,
}

#[derive(Debug, Clone)]
pub(crate) struct ConfirmAltAction {
    pub(crate) key: char,
    pub(crate) label: String,
    pub(crate) action: ConfirmAction,
}

#[derive(Debug, Clone)]
pub(crate) struct ConfirmState {
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) action: ConfirmAction,
    pub(crate) alt_action: Option<ConfirmAltAction>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectSetupState {
    pub(crate) repo_path: Option<String>,
    pub(crate) suggested_project_name: String,
    pub(crate) has_projects: bool,
    pub(crate) busy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BranchPickerMode {
    Checkout,
    ChangeTarget,
}

#[derive(Debug, Clone)]
pub(crate) struct BranchPickerState {
    pub(crate) mode: BranchPickerMode,
    pub(crate) repo_id: Uuid,
    pub(crate) repo_name: String,
    pub(crate) filter: TextFieldState,
    pub(crate) selected_index: usize,
    pub(crate) branches: Vec<GitBranchItem>,
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreateTaskFocus {
    Title,
    Description,
    Status,
    Buttons,
}

#[derive(Debug, Clone)]
pub(crate) struct CreateTaskState {
    pub(crate) title: TextFieldState,
    pub(crate) description: TextFieldState,
    pub(crate) status: TaskStatus,
    pub(crate) parent_task_id: Option<Uuid>,
    pub(crate) focus: CreateTaskFocus,
    pub(crate) selected_button: usize, // 0 = create, 1 = cancel
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LogViewMode {
    Timeline,
    Single,
}

impl Default for LogViewMode {
    fn default() -> Self {
        Self::Timeline
    }
}

impl LogViewMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Timeline => "timeline",
            Self::Single => "run",
        }
    }
}
