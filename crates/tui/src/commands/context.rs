use uuid::Uuid;

use crate::{commands::request_branch_status_refresh, state::AppState};

pub(crate) fn require_selected_attempt_id(app: &AppState) -> Result<Uuid, String> {
    app.board
        .selected_attempt_id
        .ok_or_else(|| crate::ui::messages::errors::NO_ATTEMPT_SELECTED.to_string())
}

pub(crate) fn require_repo_status_loaded(app: &mut AppState) -> Result<(), String> {
    if app.diff.repo_statuses.is_empty() {
        request_branch_status_refresh(app);
        return Err(crate::ui::messages::errors::NO_REPO_STATUS_LOADED.to_string());
    }
    Ok(())
}

pub(crate) fn ensure_attempt_selected(app: &mut AppState, context: &str) -> Option<Uuid> {
    let Some(attempt_id) = app.board.selected_attempt_id else {
        crate::ui::guards::toast_short(
            app,
            format!(
                "{context}: {}",
                crate::ui::messages::errors::NO_ATTEMPT_SELECTED
            ),
            crate::ui::palette::toast_err(),
        );
        return None;
    };
    Some(attempt_id)
}

pub(crate) fn ensure_repo_status_loaded(app: &mut AppState, context: &str) -> bool {
    if !app.diff.repo_statuses.is_empty() {
        return true;
    }
    request_branch_status_refresh(app);
    crate::ui::guards::toast_short(
        app,
        format!("{context}: loading repo status…"),
        crate::ui::palette::toast_warn(),
    );
    false
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
        crate::selection::change::set_selected_repo_index(app, idx);
    }

    let repo = app
        .diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .ok_or_else(|| crate::ui::messages::errors::NO_REPO_SELECTED.to_string())?;
    Ok((repo.repo_id, repo.repo_name.clone()))
}
