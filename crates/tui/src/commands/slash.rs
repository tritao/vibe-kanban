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
            app.ui.last_notice = Some("Opened help. (Press Esc to close)".to_string());
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
            // Accept optional --repo for consistency with other git-ish commands.
            let parsed = parse_stack_kv_flags(tokens, &["--repo"])?;
            let _ = resolve_repo_for_command(app, parsed.values.get("--repo").map(|s| s.as_str()))?;
            crate::commands::request_stack_status_refresh(app);
            app.ui.last_notice = Some("Stack: refreshing…".to_string());
            Ok(())
        }
        "enable" => {
            let (repo_id, repo_name) =
                resolve_repo_for_command(app, parse_stack_flag_value(tokens, "--repo").as_deref())?;
            crate::commands::trigger_stack_enable(app, attempt_id, repo_id);
            app.ui.last_notice = Some(format!("Stack: enabling for {repo_name}…"));
            Ok(())
        }
        "disable" => {
            let parsed = parse_stack_kv_flags(tokens, &["--repo", "--force"])?;
            let (repo_id, repo_name) =
                resolve_repo_for_command(app, parsed.values.get("--repo").map(|s| s.as_str()))?;
            let force = parsed.bools.contains("--force");
            crate::commands::trigger_stack_disable(app, attempt_id, repo_id, force);
            app.ui.last_notice = Some(format!(
                "Stack: disabling for {repo_name}{}…",
                if force { " (force)" } else { "" }
            ));
            Ok(())
        }
        "new" => {
            let parsed = parse_stack_kv_flags(tokens, &["--repo", "--name"])?;
            let message = parsed.rest.join(" ").trim().to_string();
            if message.is_empty() {
                return Err("usage: /stack new <MESSAGE> [--name N] [--repo R]".to_string());
            }
            let (repo_id, repo_name) =
                resolve_repo_for_command(app, parsed.values.get("--repo").map(|s| s.as_str()))?;
            let name = parsed.values.get("--name").cloned();
            crate::commands::trigger_stack_new(app, attempt_id, repo_id, name, message);
            app.ui.last_notice = Some(format!("Stack: new ({repo_name})…"));
            Ok(())
        }
        "refresh" => {
            let parsed = parse_stack_kv_flags(tokens, &["--repo", "--paths", "--index"])?;
            let (repo_id, repo_name) =
                resolve_repo_for_command(app, parsed.values.get("--repo").map(|s| s.as_str()))?;
            let paths = parsed
                .values
                .get("--paths")
                .map(|s| {
                    s.split(',')
                        .map(|p| p.trim().to_string())
                        .filter(|p| !p.is_empty())
                        .collect::<Vec<_>>()
                })
                .filter(|v: &Vec<String>| !v.is_empty());
            let allow_dirty_index = parsed.bools.contains("--index");
            crate::commands::trigger_stack_refresh(
                app,
                attempt_id,
                repo_id,
                paths,
                allow_dirty_index,
            );
            app.ui.last_notice = Some(format!("Stack: refresh ({repo_name})…"));
            Ok(())
        }
        "push" => {
            let (repo_id, repo_name) =
                resolve_repo_for_command(app, parse_stack_flag_value(tokens, "--repo").as_deref())?;
            crate::commands::trigger_stack_push(app, attempt_id, repo_id);
            app.ui.last_notice = Some(format!("Stack: push ({repo_name})…"));
            Ok(())
        }
        "pop" => {
            let (repo_id, repo_name) =
                resolve_repo_for_command(app, parse_stack_flag_value(tokens, "--repo").as_deref())?;
            crate::commands::trigger_stack_pop(app, attempt_id, repo_id);
            app.ui.last_notice = Some(format!("Stack: pop ({repo_name})…"));
            Ok(())
        }
        "undo" => {
            let (repo_id, repo_name) =
                resolve_repo_for_command(app, parse_stack_flag_value(tokens, "--repo").as_deref())?;
            crate::commands::trigger_stack_undo(app, attempt_id, repo_id);
            app.ui.last_notice = Some(format!("Stack: undo ({repo_name})…"));
            Ok(())
        }
        "redo" => {
            let (repo_id, repo_name) =
                resolve_repo_for_command(app, parse_stack_flag_value(tokens, "--repo").as_deref())?;
            crate::commands::trigger_stack_redo(app, attempt_id, repo_id);
            app.ui.last_notice = Some(format!("Stack: redo ({repo_name})…"));
            Ok(())
        }
        other => Err(format!("unknown stack subcommand: {other}")),
    }
}

struct StackParsedArgs {
    values: std::collections::HashMap<String, String>,
    bools: std::collections::HashSet<&'static str>,
    rest: Vec<String>,
}

fn parse_stack_flag_value(tokens: &[String], flag: &'static str) -> Option<String> {
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] == flag {
            return tokens.get(i + 1).cloned();
        }
        i += 1;
    }
    None
}

fn parse_stack_kv_flags(
    tokens: &[String],
    allowed: &[&'static str],
) -> Result<StackParsedArgs, String> {
    let mut values = std::collections::HashMap::<String, String>::new();
    let mut bools = std::collections::HashSet::<&'static str>::new();
    let mut rest: Vec<String> = vec![];

    // tokens[0]=stack, tokens[1]=subcommand, remaining is mixed flags/args.
    let mut i = 2usize;
    while i < tokens.len() {
        let t = tokens[i].as_str();
        if t.starts_with("--") {
            if !allowed.contains(&t) {
                return Err(format!("unknown flag: {t} (try /help)"));
            }
            match t {
                "--index" => {
                    if bools.contains("--index") {
                        return Err("duplicate flag: --index".to_string());
                    }
                    bools.insert("--index");
                    i += 1;
                }
                "--force" => {
                    if bools.contains("--force") {
                        return Err("duplicate flag: --force".to_string());
                    }
                    bools.insert("--force");
                    i += 1;
                }
                "--repo" | "--name" | "--paths" => {
                    let v = tokens
                        .get(i + 1)
                        .ok_or_else(|| format!("missing value for {t}"))?;
                    if v.starts_with("--") {
                        return Err(format!("missing value for {t}"));
                    }
                    if values.contains_key(t) {
                        return Err(format!("duplicate flag: {t}"));
                    }
                    values.insert(t.to_string(), v.clone());
                    i += 2;
                }
                _ => return Err(format!("unknown flag: {t} (try /help)")),
            }
            continue;
        }
        rest.push(tokens[i].clone());
        i += 1;
    }

    Ok(StackParsedArgs {
        values,
        bools,
        rest,
    })
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
        app.ui.last_notice = Some("Commits: refreshing…".to_string());
        return Ok(());
    }

    crate::commands::select_commits_mode(app);
    app.ui.last_notice = Some("Commits: loaded.".to_string());
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
        app.ui.last_notice = Some("Loading repos…".to_string());
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
        app.ui.last_notice = Some(msg.trim_end().to_string());
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
    app.ui.last_notice = Some(format!(
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
