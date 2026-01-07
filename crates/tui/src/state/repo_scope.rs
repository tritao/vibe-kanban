use uuid::Uuid;

use crate::state::{AppState, RepoBranchStatus};

pub(crate) fn selected_repo(app: &AppState) -> Option<&RepoBranchStatus> {
    app.diff.repo_statuses.get(
        app.diff
            .selected_repo_index
            .min(app.diff.repo_statuses.len().saturating_sub(1)),
    )
}

pub(crate) fn selected_repo_id(app: &AppState) -> Option<Uuid> {
    selected_repo(app).map(|r| r.repo_id)
}

pub(crate) fn selected_repo_index_clamped(app: &AppState) -> Option<usize> {
    app.diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .map(|_| {
            app.diff
                .selected_repo_index
                .min(app.diff.repo_statuses.len() - 1)
        })
}
