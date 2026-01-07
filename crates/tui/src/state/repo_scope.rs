use uuid::Uuid;

use crate::state::AppState;

pub(crate) fn selected_repo_id(app: &AppState) -> Option<Uuid> {
    app.diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .map(|r| r.repo_id)
}
