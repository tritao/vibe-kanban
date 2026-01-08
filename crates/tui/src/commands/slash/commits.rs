use super::{require_repo_status_loaded, require_selected_attempt_id};
use crate::state::AppState;

pub(super) fn handle_commits_command(app: &mut AppState) -> Result<(), String> {
    let _attempt_id = require_selected_attempt_id(app)?;
    require_repo_status_loaded(app)?;

    if let Some(repo) = crate::state::repo_scope::selected_repo(app) {
        if app
            .diff
            .stack_status_by_repo
            .get(&repo.repo_id)
            .is_some_and(|s| s.available && s.enabled)
        {
            return Err("commits view unavailable while stack mode is enabled".to_string());
        }
    }

    if app.diff.list_mode == crate::state::DiffListMode::Commits {
        crate::commands::request_commit_list_refresh(app);
        app.ui
            .set_notice(crate::ui::messages::notices::COMMITS_REFRESHING);
        return Ok(());
    }

    crate::commands::select_commits_mode(app);
    app.ui
        .set_notice(crate::ui::messages::notices::COMMITS_LOADED);
    Ok(())
}
