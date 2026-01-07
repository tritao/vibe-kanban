use super::DiffRepoAction;
use crate::{
    events::GitOpKind,
    state::{AppState, RepoBranchStatus},
};

pub(super) fn selected_attempt_branch(app: &AppState) -> String {
    app.board
        .selected_attempt_id
        .and_then(|id| app.board.attempts.iter().find(|a| a.id == id))
        .map(|a| a.branch.clone())
        .unwrap_or_else(|| "—".to_string())
}

pub(super) fn repo_index_with_conflicts(app: &AppState) -> Option<usize> {
    let selected = crate::state::repo_scope::selected_repo(app);
    if selected
        .is_some_and(|r| r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty())
    {
        return crate::state::repo_scope::selected_repo_index_clamped(app);
    }
    app.diff
        .repo_statuses
        .iter()
        .position(|r| r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty())
}

pub(super) fn git_kind_for_diff_action(action: DiffRepoAction) -> Option<GitOpKind> {
    match action {
        DiffRepoAction::RefreshStatus => Some(GitOpKind::Status),
        DiffRepoAction::Merge => Some(GitOpKind::Merge),
        DiffRepoAction::Rebase => Some(GitOpKind::Rebase),
        DiffRepoAction::CreatePr => Some(GitOpKind::CreatePr),
        DiffRepoAction::AbortConflicts => Some(GitOpKind::Abort),
        DiffRepoAction::ResolveConflicts
        | DiffRepoAction::OpenConflict
        | DiffRepoAction::OpenPr => None,
    }
}

pub(super) fn selected_repo_status(app: &AppState) -> Option<&RepoBranchStatus> {
    crate::state::repo_scope::selected_repo(app)
}
