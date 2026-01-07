use super::{
    context::{require_repo_status_loaded, require_selected_attempt_id, resolve_repo_for_command},
    git_ops::request_branch_status_refresh,
};
use crate::{
    events::NetEvent,
    net::ops::{delete_task_http, open_editor_http},
    state::AppState,
    ui::{DiffRepoAction, trigger_diff_repo_action},
};

mod submit;
pub(crate) use submit::submit_composer;
mod git;
pub(crate) use git::trigger_abort_conflicts;
mod executor;
mod model;
mod pr;

fn parse_slash_command(app: &mut AppState, tokens: &[String]) -> Result<bool, String> {
    let cmd =
        crate::slash::canonical_command_name(tokens[0].as_str()).unwrap_or(tokens[0].as_str());
    match cmd {
        "help" => {
            app.ui.show_help = true;
            app.ui.set_notice("Opened help. (Press Esc to close)");
            Ok(false)
        }
        "quit" => Ok(true),
        "status" => {
            trigger_diff_repo_action(app, DiffRepoAction::RefreshStatus);
            Ok(false)
        }
        "commits" => {
            handle_commits_command(app)?;
            Ok(false)
        }
        "files" => {
            crate::commands::select_files_mode(app);
            crate::diff_preview::schedule_diff_preview_refresh(
                app,
                std::time::Duration::from_millis(0),
            );
            Ok(false)
        }
        "stack" => {
            handle_stack_command(app, tokens)?;
            Ok(false)
        }
        "resolve" => {
            git::handle_resolve_command(app, tokens)?;
            Ok(false)
        }
        "repo" => {
            handle_repo_command(app, tokens.get(1).map(|s| s.as_str()))?;
            Ok(false)
        }
        "rebase" => {
            git::handle_rebase_command(app, tokens)?;
            Ok(false)
        }
        "abort" => {
            git::handle_abort_command(app, tokens)?;
            Ok(false)
        }
        "merge" => {
            git::handle_merge_command(app, tokens)?;
            Ok(false)
        }
        "push" => {
            git::handle_push_command(app, tokens)?;
            Ok(false)
        }
        "pr" => {
            pr::handle_pr_command(app, tokens)?;
            Ok(false)
        }
        "open" => {
            handle_open_command(app, tokens)?;
            Ok(false)
        }
        "executor" => {
            executor::handle_executor_command(app, tokens)?;
            Ok(false)
        }
        "model" => {
            model::handle_model_command(app, tokens)?;
            Ok(false)
        }
        "delete" => {
            handle_delete_command(app, tokens)?;
            Ok(false)
        }
        _ => Err(crate::slash::unknown_command_error(tokens[0].as_str())),
    }
}

fn handle_stack_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let attempt_id = require_selected_attempt_id(app)?;
    require_repo_status_loaded(app)?;

    // Default: /stack status
    let sub = tokens.get(1).map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "status" => {
            let help = crate::slash::help_syntax_for_subcommand("stack", "status")
                .unwrap_or("/stack status");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "status").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let _ = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::request_stack_status_refresh(app);
            app.ui.set_notice("Stack: refreshing…");
            Ok(())
        }
        "enable" => {
            let help = crate::slash::help_syntax_for_subcommand("stack", "enable")
                .unwrap_or("/stack enable");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "enable").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::trigger_stack_enable(app, attempt_id, repo_id);
            app.ui
                .set_notice(format!("Stack: enabling for {repo_name}…"));
            Ok(())
        }
        "disable" => {
            let help = crate::slash::help_syntax_for_subcommand("stack", "disable")
                .unwrap_or("/stack disable");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "disable").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            let force = parsed.get_bool("--force");
            crate::commands::trigger_stack_disable(app, attempt_id, repo_id, force);
            app.ui.set_notice(format!(
                "Stack: disabling for {repo_name}{}…",
                if force { " (force)" } else { "" }
            ));
            Ok(())
        }
        "new" => {
            let help =
                crate::slash::help_syntax_for_subcommand("stack", "new").unwrap_or("/stack new");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "new").unwrap_or(&[]),
                help,
            )?;
            let message = rest.join(" ").trim().to_string();
            if message.is_empty() {
                return Err(crate::slash::usage_for_command("stack")
                    .unwrap_or("usage: /stack new <MESSAGE> [--name N] [--repo R]")
                    .to_string());
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            let name = parsed.get_value("--name").map(ToString::to_string);
            crate::commands::trigger_stack_new(app, attempt_id, repo_id, name, message);
            app.ui.set_notice(format!("Stack: new ({repo_name})…"));
            Ok(())
        }
        "refresh" => {
            let help = crate::slash::help_syntax_for_subcommand("stack", "refresh")
                .unwrap_or("/stack refresh");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "refresh").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            let paths = parsed
                .get_value("--paths")
                .map(|s| {
                    s.split(',')
                        .map(|p| p.trim().to_string())
                        .filter(|p| !p.is_empty())
                        .collect::<Vec<_>>()
                })
                .filter(|v: &Vec<String>| !v.is_empty());
            let allow_dirty_index = parsed.get_bool("--index");
            crate::commands::trigger_stack_refresh(
                app,
                attempt_id,
                repo_id,
                paths,
                allow_dirty_index,
            );
            app.ui.set_notice(format!("Stack: refresh ({repo_name})…"));
            Ok(())
        }
        "push" => {
            let help =
                crate::slash::help_syntax_for_subcommand("stack", "push").unwrap_or("/stack push");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "push").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::trigger_stack_push(app, attempt_id, repo_id);
            app.ui.set_notice(format!("Stack: push ({repo_name})…"));
            Ok(())
        }
        "pop" => {
            let help =
                crate::slash::help_syntax_for_subcommand("stack", "pop").unwrap_or("/stack pop");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "pop").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::trigger_stack_pop(app, attempt_id, repo_id);
            app.ui.set_notice(format!("Stack: pop ({repo_name})…"));
            Ok(())
        }
        "undo" => {
            let help =
                crate::slash::help_syntax_for_subcommand("stack", "undo").unwrap_or("/stack undo");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "undo").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::trigger_stack_undo(app, attempt_id, repo_id);
            app.ui.set_notice(format!("Stack: undo ({repo_name})…"));
            Ok(())
        }
        "redo" => {
            let help =
                crate::slash::help_syntax_for_subcommand("stack", "redo").unwrap_or("/stack redo");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "redo").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::trigger_stack_redo(app, attempt_id, repo_id);
            app.ui.set_notice(format!("Stack: redo ({repo_name})…"));
            Ok(())
        }
        other => Err(format!("unknown stack subcommand: {other}")),
    }
}

fn handle_commits_command(app: &mut AppState) -> Result<(), String> {
    let _attempt_id = require_selected_attempt_id(app)?;
    require_repo_status_loaded(app)?;

    if let Some(repo) = app.diff.repo_statuses.get(app.diff.selected_repo_index) {
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
        app.ui.set_notice("Commits: refreshing…");
        return Ok(());
    }

    crate::commands::select_commits_mode(app);
    app.ui.set_notice("Commits: loaded.");
    Ok(())
}

fn handle_delete_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let Some(task_id) = app.board.selected_task_id else {
        return Err("no task selected".to_string());
    };

    let help = crate::slash::help_syntax_for_command("delete").unwrap_or("/delete [--subtree]");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("delete"), help)?;
    let mode = if parsed.get_bool("--subtree") {
        Some("subtree")
    } else {
        Some("promote")
    };

    crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
        match delete_task_http(&base_url, task_id, mode).await {
            Ok(()) => {
                let _ = net_tx
                    .send(NetEvent::Notice("Deleted task.".to_string()))
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("delete task failed: {e}")))
                    .await;
            }
        }
    });

    Ok(())
}

fn handle_repo_command(app: &mut AppState, arg: Option<&str>) -> Result<(), String> {
    if app.diff.repo_statuses.is_empty() {
        request_branch_status_refresh(app);
        app.ui.set_notice("Loading repos…");
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
    crate::selection_hooks::set_selected_repo_index(app, idx);
    app.ui.set_notice(format!(
        "Selected repo: {}",
        app.diff.repo_statuses[idx].repo_name
    ));
    Ok(())
}

fn handle_open_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    if tokens.len() < 2 {
        return Err(crate::slash::usage_for_command("open")
            .unwrap_or("usage: /open <file_path>")
            .to_string());
    }
    let attempt_id = require_selected_attempt_id(app)?;
    let file_path = tokens[1].clone();
    crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
        match open_editor_http(&base_url, attempt_id, Some(file_path.clone())).await {
            Ok(url) => {
                let msg = match url {
                    Some(url) => format!("Opened editor for {file_path}: {url}"),
                    None => format!("Opened editor for {file_path}."),
                };
                let _ = net_tx.send(NetEvent::Notice(msg)).await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("open editor failed: {e}")))
                    .await;
            }
        }
    });
    Ok(())
}
