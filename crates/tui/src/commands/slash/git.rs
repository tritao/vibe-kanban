use uuid::Uuid;

use crate::{
    commands::{require_repo_status_loaded, require_selected_attempt_id, resolve_repo_for_command},
    events::GitOpKind,
    net::ops::{
        abort_conflicts_http, force_push_task_attempt_branch_http, merge_task_attempt_http,
        push_task_attempt_branch_http, rebase_task_attempt_http,
    },
    state::AppState,
    ui::DiffRepoAction,
};

pub(crate) fn trigger_abort_conflicts(
    app: &mut AppState,
    attempt_id: Uuid,
    repo_id: Uuid,
    repo_name: &str,
) {
    crate::commands::spawn_repo_git_op(
        app,
        attempt_id,
        repo_id,
        GitOpKind::Abort,
        repo_name,
        crate::commands::GitOpOutcome {
            notice: Some(format!("Aborted conflicts for {repo_name}.")),
            refresh_branch_status: true,
            diff_reconnect: false,
            finished_message_ok: Some(format!("Git: abort finished ({repo_name})")),
            finished_message_err: Some(format!("Git: abort failed ({repo_name})")),
        },
        move |base_url| async move { abort_conflicts_http(&base_url, attempt_id, repo_id).await },
    );
}

pub(super) fn handle_abort_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("abort").unwrap_or("/abort");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("abort"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    let attempt_id = require_selected_attempt_id(app)?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    trigger_abort_conflicts(app, attempt_id, repo_id, &repo_name);
    Ok(())
}

pub(super) fn handle_resolve_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("resolve").unwrap_or("/resolve [--repo R]");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("resolve"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    let _attempt_id = require_selected_attempt_id(app)?;
    require_repo_status_loaded(app)?;

    if let Some(arg) = repo_arg.as_deref().filter(|s| !s.trim().is_empty()) {
        let _ = resolve_repo_for_command(app, Some(arg))?;
        let ok = app
            .diff
            .repo_statuses
            .get(app.diff.selected_repo_index)
            .is_some_and(|r| {
                r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty()
            });
        if !ok {
            return Err(format!("repo has no conflicts: {arg}"));
        }
    }
    crate::ui::trigger_diff_repo_action(app, DiffRepoAction::ResolveConflicts);
    Ok(())
}

pub(super) fn handle_rebase_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("rebase").unwrap_or("/rebase [--onto B]");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("rebase"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);
    let onto = parsed.get_value("--onto").map(ToString::to_string);
    let old = parsed.get_value("--old").map(ToString::to_string);

    let attempt_id = require_selected_attempt_id(app)?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    if let Some(r) = app.diff.repo_statuses.iter().find(|r| r.repo_id == repo_id) {
        if r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty() {
            crate::ui::toasts::warn_short(
                app,
                "Rebase: conflicts in progress (resolve/abort first)",
            );
            return Ok(());
        }
        if old.is_none() && onto.is_none() && r.status.commits_behind.unwrap_or(0) == 0 {
            crate::ui::toasts::ok_short(app, "Rebase: already up to date");
            return Ok(());
        }
    }

    crate::commands::spawn_repo_git_op(
        app,
        attempt_id,
        repo_id,
        GitOpKind::Rebase,
        &repo_name,
        crate::commands::GitOpOutcome {
            notice: Some(format!("Rebase started for {repo_name}.")),
            refresh_branch_status: true,
            diff_reconnect: true,
            finished_message_ok: Some(format!("Git: rebase finished ({repo_name})")),
            finished_message_err: Some(format!("Git: rebase failed ({repo_name})")),
        },
        move |base_url| async move {
            rebase_task_attempt_http(&base_url, attempt_id, repo_id, old, onto).await
        },
    );
    Ok(())
}

pub(super) fn handle_merge_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("merge").unwrap_or("/merge");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("merge"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    let attempt_id = require_selected_attempt_id(app)?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    if let Some(r) = app.diff.repo_statuses.iter().find(|r| r.repo_id == repo_id) {
        if r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty() {
            crate::ui::toasts::warn_short(
                app,
                "Merge: conflicts in progress (resolve/abort first)",
            );
            return Ok(());
        }
        if r.status.commits_ahead.unwrap_or(0) == 0 {
            crate::ui::toasts::ok_short(app, "Merge: nothing to merge (up to date)");
            return Ok(());
        }
    }

    crate::commands::spawn_repo_git_op(
        app,
        attempt_id,
        repo_id,
        GitOpKind::Merge,
        &repo_name,
        crate::commands::GitOpOutcome {
            notice: Some(format!("Merged {repo_name}.")),
            refresh_branch_status: true,
            diff_reconnect: false,
            finished_message_ok: Some(format!("Git: merge finished ({repo_name})")),
            finished_message_err: Some(format!("Git: merge failed ({repo_name})")),
        },
        move |base_url| async move { merge_task_attempt_http(&base_url, attempt_id, repo_id).await },
    );
    Ok(())
}

pub(super) fn handle_push_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("push").unwrap_or("/push [--force]");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("push"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);
    let force = parsed.get_bool("--force");

    let attempt_id = require_selected_attempt_id(app)?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    let kind = if force {
        GitOpKind::ForcePush
    } else {
        GitOpKind::Push
    };

    let notice = format!("Pushed {repo_name}{}.", if force { " (force)" } else { "" });
    let ok_message = format!(
        "Git: push finished ({repo_name}{})",
        if force { ", force" } else { "" }
    );
    let err_message = format!("Git: push failed ({repo_name})");
    crate::commands::spawn_repo_git_op(
        app,
        attempt_id,
        repo_id,
        kind,
        &repo_name,
        crate::commands::GitOpOutcome {
            notice: Some(notice),
            refresh_branch_status: true,
            diff_reconnect: false,
            finished_message_ok: Some(ok_message),
            finished_message_err: Some(err_message),
        },
        move |base_url| async move {
            if force {
                force_push_task_attempt_branch_http(&base_url, attempt_id, repo_id).await
            } else {
                push_task_attempt_branch_http(&base_url, attempt_id, repo_id).await
            }
        },
    );
    Ok(())
}
