pub(crate) mod button_row;
pub(crate) mod chrome;
pub(crate) mod components;
pub(crate) mod create_task;
pub(crate) mod diff;
pub(crate) mod execution;
pub(crate) mod layout;
pub(crate) mod modals;
pub(crate) mod palette;
pub(crate) mod scroll;
pub(crate) mod viewport;
pub(crate) mod widgets;

pub(crate) use chrome::{render_bottom_bar, render_top_bar};
pub(crate) use components::{
    board_pane::render_board_pane,
    diff_repo_bar::{DiffRepoAction, trigger_diff_repo_action},
};
pub(crate) use create_task::{
    handle_create_task_key, open_create_task_modal, render_create_task_modal,
};
pub(crate) use diff::{render_diff_pane, sync_selected_repo_from_diff_selection};
pub(crate) use execution::{
    close_composer, open_composer, render_composer_autocomplete, render_execution_pane,
};
