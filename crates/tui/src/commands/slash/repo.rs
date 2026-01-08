use crate::{commands::git_ops::request_branch_status_refresh, state::AppState};

pub(super) fn handle_repo_command(app: &mut AppState, arg: Option<&str>) -> Result<(), String> {
    if app.diff.repo_statuses.is_empty() {
        request_branch_status_refresh(app);
        return Ok(());
    }

    let Some(arg) = arg.filter(|s| !s.trim().is_empty()) else {
        let mut msg = String::new();
        msg.push_str("Repos:\n");
        for (idx, repo) in app.diff.repo_statuses.iter().enumerate() {
            let marker = if idx == app.diff.selected_repo_index {
                "*"
            } else {
                " "
            };
            msg.push_str(&format!("  {marker} {}. {}\n", idx + 1, repo.repo_name));
        }
        app.ui.set_notice(msg.trim_end().to_string());
        return Ok(());
    };

    let idx = if let Ok(n) = arg.parse::<usize>() {
        n.saturating_sub(1)
    } else {
        let needle = arg.to_ascii_lowercase();
        app.diff
            .repo_statuses
            .iter()
            .position(|r| r.repo_name.to_ascii_lowercase() == needle)
            .or_else(|| {
                app.diff
                    .repo_statuses
                    .iter()
                    .position(|r| r.repo_name.to_ascii_lowercase().contains(&needle))
            })
            .ok_or_else(|| format!("unknown repo: {arg}"))?
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
