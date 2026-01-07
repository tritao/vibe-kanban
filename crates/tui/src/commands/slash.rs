use std::time::{Duration, Instant};

use ratatui::style::Color;
use uuid::Uuid;

use super::{
    context::{require_repo_status_loaded, require_selected_attempt_id, resolve_repo_for_command},
    git_ops::{begin_git_op, request_branch_status_refresh, set_toast},
};
use crate::{
    commands::open_url,
    events::{GitOpKind, NetEvent},
    net::ops::{
        CreateGitHubPrRequest, abort_conflicts_http, attach_pr_http, branch_status_http,
        create_pr_http, delete_task_http, force_push_task_attempt_branch_http,
        get_pr_comments_http, merge_task_attempt_http, open_editor_http,
        push_task_attempt_branch_http, rebase_task_attempt_http, update_executor_profile_http,
        update_model_settings_http,
    },
    selection::find_task,
    state::{AppState, Merge},
    ui::{DiffRepoAction, trigger_diff_repo_action},
};

mod submit;
pub(crate) use submit::submit_composer;
mod git;
pub(crate) use git::trigger_abort_conflicts;

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
            handle_pr_command(app, tokens)?;
            Ok(false)
        }
        "open" => {
            handle_open_command(app, tokens)?;
            Ok(false)
        }
        "executor" => {
            handle_executor_command(app, tokens)?;
            Ok(false)
        }
        "model" => {
            handle_model_command(app, tokens)?;
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

fn handle_executor_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    if tokens.len() == 1 {
        let current = app
            .ui
            .selected_executor_profile
            .as_ref()
            .map(|p| match p.variant.as_deref() {
                Some(v) if !v.trim().is_empty() => format!("{}:{v}", p.executor),
                _ => p.executor.clone(),
            })
            .unwrap_or_else(|| "unknown".to_string());

        if app.ui.available_executors.is_empty() {
            return Err(
                "no executor profiles loaded yet (try reconnect or wait for /api/info)".to_string(),
            );
        }
        let list = app.ui.available_executors.join(", ");
        app.ui.last_notice = Some(format!("Executor: {current}\nAvailable: {list}"));
        return Ok(());
    }

    let exec = tokens[1].trim();
    if exec.is_empty() {
        return Err("usage: /executor <NAME> [--variant V]".to_string());
    }

    let help = crate::slash::help_syntax_for_command("executor")
        .unwrap_or("/executor <name> [--variant V]");
    let parsed =
        crate::slash::parse_flags(tokens, 2, crate::slash::flags_for_command("executor"), help)?;
    let variant = parsed.get_value("--variant").map(ToString::to_string);

    if !app.ui.available_executors.is_empty()
        && !app
            .ui
            .available_executors
            .iter()
            .any(|e| e.eq_ignore_ascii_case(exec))
    {
        return Err(format!("unknown executor: {exec} (try /executor)"));
    }

    let selection = crate::state::ExecutorProfileSelection {
        executor: exec.to_string(),
        variant: variant.clone().filter(|s| !s.trim().is_empty()),
    };
    app.ui.selected_executor_profile = Some(selection.clone());
    app.ui.last_notice = Some(format!(
        "Setting executor profile: {}{}",
        selection.executor,
        selection
            .variant
            .as_deref()
            .map(|v| format!(":{v}"))
            .unwrap_or_default()
    ));

    crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
        match update_executor_profile_http(&base_url, &selection).await {
            Ok(()) => {
                let _ = net_tx
                    .send(NetEvent::Notice("Executor profile updated.".to_string()))
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!(
                        "failed to update executor profile: {e}"
                    )))
                    .await;
            }
        }
    });

    Ok(())
}

fn current_model_and_effort(
    profiles_executors: &serde_json::Value,
    selection: &crate::state::ExecutorProfileSelection,
) -> (Option<String>, Option<String>) {
    let Some(execs) = profiles_executors.as_object() else {
        return (None, None);
    };
    let exec_key = execs
        .keys()
        .find(|k| k.eq_ignore_ascii_case(&selection.executor))
        .cloned()
        .unwrap_or_else(|| selection.executor.clone());
    let Some(variants) = execs.get(&exec_key).and_then(|v| v.as_object()) else {
        return (None, None);
    };
    let wanted_variant = selection.variant.as_deref().unwrap_or("DEFAULT");
    let variant_key = variants
        .keys()
        .find(|k| k.eq_ignore_ascii_case(wanted_variant))
        .cloned()
        .unwrap_or_else(|| wanted_variant.to_string());
    let Some(variant) = variants.get(&variant_key).and_then(|v| v.as_object()) else {
        return (None, None);
    };
    let nested_key = variant
        .keys()
        .find(|k| k.eq_ignore_ascii_case(&exec_key))
        .cloned()
        .unwrap_or_else(|| exec_key.clone());
    let Some(cfg) = variant.get(&nested_key).and_then(|v| v.as_object()) else {
        return (None, None);
    };

    let model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let effort = cfg
        .get("model_reasoning_effort")
        .or_else(|| cfg.get("reasoning_effort"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    (model, effort)
}

fn handle_model_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let Some(selection) = app.ui.selected_executor_profile.clone() else {
        return Err("no executor selected (try /executor)".to_string());
    };

    if tokens.len() == 1 {
        let (model, effort) = current_model_and_effort(&app.ui.executor_profiles, &selection);
        let model = model.unwrap_or_else(|| "unset".to_string());
        let effort = effort.unwrap_or_else(|| "unset".to_string());
        app.ui.last_notice = Some(format!(
            "Model: {model}\nReasoning effort: {effort}\n\nSet: /model <MODEL> [--effort E]"
        ));
        return Ok(());
    }

    let mut model: Option<String> = None;
    let mut flags_start = 1;
    if let Some(tok) = tokens.get(1).map(|s| s.as_str()) {
        if !tok.starts_with("--") {
            model = Some(tok.to_string());
            flags_start = 2;
        }
    }

    let help =
        crate::slash::help_syntax_for_command("model").unwrap_or("/model <MODEL> [--effort E]");
    let parsed = crate::slash::parse_flags(
        tokens,
        flags_start,
        crate::slash::flags_for_command("model"),
        help,
    )?;
    let effort = parsed.get_value("--effort").map(ToString::to_string);

    let model_empty = model
        .as_deref()
        .map(|m| m.trim().is_empty())
        .unwrap_or(true);
    let effort_empty = effort
        .as_deref()
        .map(|e| e.trim().is_empty())
        .unwrap_or(true);
    if model_empty && effort_empty {
        return Err("usage: /model <MODEL> [--effort E]".to_string());
    }

    let desc = match (model.as_deref(), effort.as_deref()) {
        (Some(m), Some(e)) => format!("{m} (effort {e})"),
        (Some(m), None) => m.to_string(),
        (None, Some(e)) => format!("(effort {e})"),
        (None, None) => "unknown".to_string(),
    };
    app.ui.last_notice = Some(format!(
        "Updating model settings for {}{}: {desc}",
        selection.executor,
        selection
            .variant
            .as_deref()
            .map(|v| format!(":{v}"))
            .unwrap_or_default()
    ));

    let selection2 = selection.clone();
    crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
        match update_model_settings_http(
            &base_url,
            &selection2,
            model.as_deref(),
            effort.as_deref(),
        )
        .await
        {
            Ok(()) => {
                let info_tx = net_tx.clone();
                let info_url = base_url.clone();
                let _ = net_tx
                    .send(NetEvent::Notice("Model settings updated.".to_string()))
                    .await;
                tokio::spawn(crate::net::load_info_task(info_url, info_tx));
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!(
                        "failed to update model settings: {e}"
                    )))
                    .await;
            }
        }
    });

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

#[allow(dead_code)]
fn trigger_abort_conflicts_legacy(
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

#[allow(dead_code)]
fn handle_abort_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("abort").unwrap_or("/abort");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("abort"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    let attempt_id = require_selected_attempt_id(app)?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    trigger_abort_conflicts_legacy(app, attempt_id, repo_id, &repo_name);
    Ok(())
}

#[allow(dead_code)]
fn handle_resolve_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
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
    trigger_diff_repo_action(app, DiffRepoAction::ResolveConflicts);
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

#[allow(dead_code)]
fn handle_rebase_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
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
            set_toast(
                app,
                "Rebase: conflicts in progress (resolve/abort first)".to_string(),
                Color::Yellow,
                Some(Instant::now() + Duration::from_secs(2)),
            );
            return Ok(());
        }
        if old.is_none() && onto.is_none() && r.status.commits_behind.unwrap_or(0) == 0 {
            set_toast(
                app,
                "Rebase: already up to date".to_string(),
                Color::Green,
                Some(Instant::now() + Duration::from_secs(2)),
            );
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

#[allow(dead_code)]
fn handle_merge_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("merge").unwrap_or("/merge");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("merge"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    let attempt_id = require_selected_attempt_id(app)?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    if let Some(r) = app.diff.repo_statuses.iter().find(|r| r.repo_id == repo_id) {
        if r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty() {
            set_toast(
                app,
                "Merge: conflicts in progress (resolve/abort first)".to_string(),
                Color::Yellow,
                Some(Instant::now() + Duration::from_secs(2)),
            );
            return Ok(());
        }
        if r.status.commits_ahead.unwrap_or(0) == 0 {
            set_toast(
                app,
                "Merge: nothing to merge (up to date)".to_string(),
                Color::Green,
                Some(Instant::now() + Duration::from_secs(2)),
            );
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

#[allow(dead_code)]
fn handle_push_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
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

fn handle_pr_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    if tokens.len() < 2 {
        return Err(crate::slash::usage_for_command("pr")
            .unwrap_or("usage: /pr <create|attach|comments|open>")
            .to_string());
    }
    match tokens[1].as_str() {
        "create" => handle_pr_create_command(app, tokens),
        "attach" => handle_pr_attach_command(app, tokens),
        "comments" => handle_pr_comments_command(app, tokens),
        "open" => handle_pr_open_command(app, tokens),
        other => Err(crate::slash::unknown_subcommand_error("pr", other)),
    }
}

fn handle_pr_open_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_subcommand("pr", "open").unwrap_or("/pr open");
    let parsed = crate::slash::parse_flags(
        tokens,
        2,
        crate::slash::flags_for_subcommand("pr", "open").unwrap_or(&[]),
        help,
    )?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    let (_repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;
    let repo = app
        .diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .ok_or_else(|| "no repo selected".to_string())?;
    let pr = repo.status.merges.iter().find_map(|m| match m {
        Merge::Pr(pr) => Some(&pr.pr_info),
        _ => None,
    });
    let Some(pr) = pr else {
        return Err("no PR attached for selected repo".to_string());
    };
    if pr.url.trim().is_empty() {
        return Err("PR has no URL".to_string());
    }

    match open_url(&pr.url) {
        Ok(()) => {
            set_toast(
                app,
                format!("PR: opened (PR#{}, {repo_name})", pr.number),
                Color::Green,
                Some(Instant::now() + Duration::from_secs(2)),
            );
            Ok(())
        }
        Err(e) => {
            app.ui.last_notice = Some(format!("PR URL: {}", pr.url));
            Err(format!("failed to open PR URL: {e}"))
        }
    }
}

fn handle_pr_create_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help =
        crate::slash::help_syntax_for_subcommand("pr", "create").unwrap_or("/pr create --title T");
    let parsed = crate::slash::parse_flags(
        tokens,
        2,
        crate::slash::flags_for_subcommand("pr", "create").unwrap_or(&[]),
        help,
    )?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);
    let title: Option<String> = parsed.get_value("--title").map(ToString::to_string);
    let body: Option<String> = parsed.get_value("--body").map(ToString::to_string);
    let base: Option<String> = parsed.get_value("--base").map(ToString::to_string);
    let draft = Some(parsed.get_bool("--draft"));
    let auto_desc = parsed.get_bool("--auto-desc");

    let attempt_id = require_selected_attempt_id(app)?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    if let Some(r) = app.diff.repo_statuses.iter().find(|r| r.repo_id == repo_id) {
        if r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty() {
            set_toast(
                app,
                "PR: conflicts in progress (resolve/abort first)".to_string(),
                Color::Yellow,
                Some(Instant::now() + Duration::from_secs(2)),
            );
            return Ok(());
        }
        if r.status.commits_ahead.unwrap_or(0) == 0 {
            set_toast(
                app,
                "PR: no changes to open (up to date)".to_string(),
                Color::Green,
                Some(Instant::now() + Duration::from_secs(2)),
            );
            return Ok(());
        }
        let pr_open = r.status.merges.iter().find_map(|m| match m {
            Merge::Pr(pr) => Some(pr.pr_info.number),
            _ => None,
        });
        if let Some(n) = pr_open {
            set_toast(
                app,
                format!("PR: already exists (PR#{n})"),
                Color::Green,
                Some(Instant::now() + Duration::from_secs(2)),
            );
            return Ok(());
        }
    }

    let title = title
        .or_else(|| {
            app.board
                .selected_task_id
                .and_then(|id| find_task(&app.board.tasks_store, id).map(|t| t.title))
        })
        .ok_or_else(|| "missing --title and no task selected".to_string())?;

    if !begin_git_op(app, Some(repo_id), GitOpKind::CreatePr, &repo_name) {
        return Ok(());
    }

    crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
        match create_pr_http(
            &base_url,
            attempt_id,
            CreateGitHubPrRequest {
                title,
                body,
                target_branch: base,
                draft,
                repo_id,
                auto_generate_description: auto_desc,
            },
        )
        .await
        {
            Ok(url) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!(
                        "PR created for {repo_name}: {url}"
                    )))
                    .await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx
                        .send(NetEvent::BranchStatusLoaded {
                            attempt_id,
                            statuses,
                        })
                        .await;
                }
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::CreatePr,
                        ok: true,
                        message: format!("Git: PR created ({repo_name})"),
                    })
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("pr create failed: {e}")))
                    .await;
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::CreatePr,
                        ok: false,
                        message: format!("Git: PR create failed ({repo_name})"),
                    })
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_pr_attach_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_subcommand("pr", "attach").unwrap_or("/pr attach");
    let parsed = crate::slash::parse_flags(
        tokens,
        2,
        crate::slash::flags_for_subcommand("pr", "attach").unwrap_or(&[]),
        help,
    )?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    let attempt_id = require_selected_attempt_id(app)?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    if !begin_git_op(app, Some(repo_id), GitOpKind::AttachPr, &repo_name) {
        return Ok(());
    }

    crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
        match attach_pr_http(&base_url, attempt_id, repo_id).await {
            Ok(resp) => {
                let msg = if resp.pr_attached {
                    if let Some(url) = resp.pr_url {
                        format!("Attached PR for {repo_name}: {url}")
                    } else {
                        format!("Attached PR for {repo_name}.")
                    }
                } else {
                    format!("No PR found to attach for {repo_name}.")
                };
                let _ = net_tx.send(NetEvent::Notice(msg)).await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx
                        .send(NetEvent::BranchStatusLoaded {
                            attempt_id,
                            statuses,
                        })
                        .await;
                }
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::AttachPr,
                        ok: true,
                        message: format!("Git: attach PR finished ({repo_name})"),
                    })
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("pr attach failed: {e}")))
                    .await;
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::AttachPr,
                        ok: false,
                        message: format!("Git: attach PR failed ({repo_name})"),
                    })
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_pr_comments_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_subcommand("pr", "comments").unwrap_or("/pr comments");
    let parsed = crate::slash::parse_flags(
        tokens,
        2,
        crate::slash::flags_for_subcommand("pr", "comments").unwrap_or(&[]),
        help,
    )?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    let attempt_id = require_selected_attempt_id(app)?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    if !begin_git_op(app, Some(repo_id), GitOpKind::PrComments, &repo_name) {
        return Ok(());
    }

    crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
        match get_pr_comments_http(&base_url, attempt_id, repo_id).await {
            Ok(count) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!(
                        "Fetched {count} PR comments for {repo_name}."
                    )))
                    .await;
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::PrComments,
                        ok: true,
                        message: format!("Git: PR comments fetched ({repo_name})"),
                    })
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("pr comments failed: {e}")))
                    .await;
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::PrComments,
                        ok: false,
                        message: format!("Git: PR comments failed ({repo_name})"),
                    })
                    .await;
            }
        }
    });
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
