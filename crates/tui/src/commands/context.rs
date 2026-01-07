use uuid::Uuid;

use crate::{commands::request_branch_status_refresh, selection_hooks, state::AppState};

pub(crate) fn require_selected_attempt_id(app: &AppState) -> Result<Uuid, String> {
    app.board
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())
}

pub(crate) fn require_repo_status_loaded(app: &mut AppState) -> Result<(), String> {
    if app.diff.repo_statuses.is_empty() {
        request_branch_status_refresh(app);
        return Err("no repo status loaded yet (run /status)".to_string());
    }
    Ok(())
}

pub(crate) fn resolve_repo_for_command(
    app: &mut AppState,
    repo_arg: Option<&str>,
) -> Result<(Uuid, String), String> {
    require_repo_status_loaded(app)?;

    if let Some(arg) = repo_arg.filter(|s| !s.trim().is_empty()) {
        if let Ok(n) = arg.parse::<usize>() {
            let idx = n.saturating_sub(1);
            let repo = app
                .diff
                .repo_statuses
                .get(idx)
                .ok_or_else(|| format!("repo index out of range: {arg}"))?;
            return Ok((repo.repo_id, repo.repo_name.clone()));
        }

        let needle = arg.to_ascii_lowercase();
        let idx = app
            .diff
            .repo_statuses
            .iter()
            .position(|r| r.repo_name.to_ascii_lowercase() == needle)
            .or_else(|| {
                app.diff
                    .repo_statuses
                    .iter()
                    .position(|r| r.repo_name.to_ascii_lowercase().contains(&needle))
            })
            .ok_or_else(|| format!("unknown repo: {arg}"))?;
        selection_hooks::set_selected_repo_index(app, idx);
    }

    let repo = app
        .diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .ok_or_else(|| "no repo selected".to_string())?;
    Ok((repo.repo_id, repo.repo_name.clone()))
}
