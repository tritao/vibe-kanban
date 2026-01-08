use crate::{
    state::AppState,
    store::repo_status::{GitActionBlock, GitActionBlockSeverity},
    ui::toasts,
};

pub(crate) fn git_action_block_seconds(app: &mut AppState, block: GitActionBlock, seconds: u64) {
    match block.severity {
        GitActionBlockSeverity::Ok => toasts::ok_seconds(app, block.message, seconds),
        GitActionBlockSeverity::Warn => toasts::warn_seconds(app, block.message, seconds),
    }
}
