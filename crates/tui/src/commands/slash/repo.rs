use crate::{
    commands::{git_ops::request_branch_status_refresh, resolve_repo_for_command},
    state::AppState,
};

pub(super) fn handle_repo_command(app: &mut AppState, arg: Option<&str>) -> Result<(), String> {
    if app.diff.repo_statuses.is_empty() {
        request_branch_status_refresh(app);
        return Ok(());
    }

    let Some(arg) = arg.filter(|s| !s.trim().is_empty()) else {
        let msg = crate::store::git_status::RepoStatuses::new(&app.diff.repo_statuses)
            .list_lines(app.diff.selected_repo_index)
            .join("\n");
        app.ui.set_notice(msg);
        return Ok(());
    };

    let idx = if let Ok(n) = arg.parse::<usize>() {
        n.saturating_sub(1)
    } else {
        let _ = resolve_repo_for_command(app, Some(arg))?;
        app.diff.selected_repo_index
    };

    if idx >= app.diff.repo_statuses.len() {
        return Err(format!("repo index out of range: {arg}"));
    }
    crate::selection::change::set_selected_repo_index(app, idx);
    app.ui.set_notice(format!(
        "Selected repo: {}",
        app.diff.repo_statuses[idx].repo_name
    ));
    Ok(())
}
