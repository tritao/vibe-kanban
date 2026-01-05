use std::time::{Duration, Instant};

use ratatui::style::Color;
use uuid::Uuid;

use super::git_ops::{
    arm_branch_status_refresh_after_next_exec, arm_branch_status_refresh_for_exec, begin_git_op,
    request_branch_status_refresh, set_toast,
};
use crate::{
    commands::open_url,
    events::{GitOpKind, NetEvent},
    logs::{append_local_user_message, set_pending_user_log},
    net::ops::{
        CreateGitHubPrRequest, abort_conflicts_http, attach_pr_http, branch_status_http,
        create_pr_http, follow_up_http, force_push_task_attempt_branch_http, get_pr_comments_http,
        latest_session_id_http, merge_task_attempt_http, open_editor_http,
        push_task_attempt_branch_http, queue_follow_up_http, rebase_task_attempt_http,
        update_executor_profile_http, update_model_settings_http,
    },
    selection::{exec_list, find_task},
    state::{AppState, Merge},
    ui::{DiffRepoAction, trigger_diff_repo_action},
};

fn is_quit_slash(message: &str) -> bool {
    let trimmed = message.trim_start();
    let Some(cmdline) = trimmed.strip_prefix('/') else {
        return false;
    };
    let tokens = crate::cli_parse::tokenize_command_line(cmdline).ok();
    let Some(tokens) = tokens else {
        return false;
    };
    let Some(first) = tokens.get(0) else {
        return false;
    };
    let cmd = crate::slash::canonical_command_name(first.as_str()).unwrap_or(first.as_str());
    cmd == "quit"
}

pub(crate) fn submit_composer(app: &mut AppState) -> bool {
    let msg = app.ui.composer.buffer.trim_end().to_string();
    if msg.trim().is_empty() {
        app.ui.composer_active = false;
        app.ui.composer.clear();
        return false;
    }

    let refresh_branch_status_after_send = app.ui.refresh_branch_status_after_send;
    app.ui.refresh_branch_status_after_send = false;

    // Keep a local record of what the user sent in the run logs, since the backend log stream
    // does not always include user messages.
    let mut execs_for_log = exec_list(&app.exec.exec_store);
    execs_for_log.sort_by_key(|e| e.created_at.clone().unwrap_or_default());
    let current_exec_id = app
        .exec
        .selected_exec_id
        .or_else(|| execs_for_log.last().map(|e| e.id));

    app.ui.composer_active = false;
    app.ui.composer.clear();

    if crate::slash::composer_is_slash_mode(&msg) {
        if is_quit_slash(&msg) {
            return true;
        }
        if let Some(exec_id) = current_exec_id {
            append_local_user_message(app, exec_id, &msg);
        }
        return submit_slash_command(app, &msg);
    }

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    let attempt_id = app.board.selected_attempt_id;

    let execs = exec_list(&app.exec.exec_store);
    let active = app
        .exec
        .selected_exec_id
        .and_then(|id| execs.iter().find(|e| e.id == id));
    let session_id = active.and_then(|e| e.session_id);
    let is_running = active.and_then(|e| e.status.as_deref()) == Some("running");

    if let Some(exec_id) = current_exec_id {
        if is_running {
            append_local_user_message(app, exec_id, &msg);
        } else {
            set_pending_user_log(app, msg.clone());
        }
    }
    if refresh_branch_status_after_send {
        if is_running {
            if let Some(exec_id) = current_exec_id {
                arm_branch_status_refresh_for_exec(app, exec_id);
            }
        } else {
            arm_branch_status_refresh_after_next_exec(app, current_exec_id);
        }
    }

    tokio::spawn(async move {
        let session_id = match session_id {
            Some(id) => Some(id),
            None => match attempt_id {
                Some(workspace_id) => match latest_session_id_http(&base_url, workspace_id).await {
                    Ok(id) => id,
                    Err(e) => {
                        let _ = net_tx
                            .send(NetEvent::Error(format!("failed to load sessions: {e}")))
                            .await;
                        return;
                    }
                },
                None => None,
            },
        };

        let Some(session_id) = session_id else {
            let _ = net_tx
                .send(NetEvent::Error(
                    "no session available for this attempt".to_string(),
                ))
                .await;
            return;
        };

        let result = if is_running {
            queue_follow_up_http(&base_url, session_id, &msg).await
        } else {
            follow_up_http(&base_url, session_id, &msg).await
        };

        match result {
            Ok(()) => {}
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("follow-up failed: {e}")))
                    .await;
            }
        }
    });

    false
}

pub(crate) fn submit_slash_command(app: &mut AppState, raw: &str) -> bool {
    let cmdline = raw.trim_start().trim_start_matches('/');
    let tokens = match crate::cli_parse::tokenize_command_line(cmdline) {
        Ok(t) => t,
        Err(e) => {
            app.ui.last_error = Some(format!("invalid command: {e}"));
            return false;
        }
    };

    if tokens.is_empty() {
        app.ui.last_error = Some("invalid command: empty".to_string());
        return false;
    }

    match parse_slash_command(app, &tokens) {
        Ok(quit) => return quit,
        Err(e) => {
            app.ui.last_error = Some(e);
        }
    }
    false
}

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
        "resolve" => {
            handle_resolve_command(app, tokens)?;
            Ok(false)
        }
        "repo" => {
            handle_repo_command(app, tokens.get(1).map(|s| s.as_str()))?;
            Ok(false)
        }
        "rebase" => {
            handle_rebase_command(app, tokens)?;
            Ok(false)
        }
        "abort" => {
            handle_abort_command(app, tokens)?;
            Ok(false)
        }
        "merge" => {
            handle_merge_command(app, tokens)?;
            Ok(false)
        }
        "push" => {
            handle_push_command(app, tokens)?;
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
        _ => Err(crate::slash::unknown_command_error(tokens[0].as_str())),
    }
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

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
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

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    let selection2 = selection.clone();
    tokio::spawn(async move {
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

pub(crate) fn trigger_abort_conflicts(
    app: &mut AppState,
    attempt_id: Uuid,
    repo_id: Uuid,
    repo_name: &str,
) {
    if !begin_git_op(app, Some(repo_id), GitOpKind::Abort, repo_name) {
        return;
    }

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    let repo_name = repo_name.to_string();
    tokio::spawn(async move {
        match abort_conflicts_http(&base_url, attempt_id, repo_id).await {
            Ok(()) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!(
                        "Aborted conflicts for {repo_name}."
                    )))
                    .await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                }
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::Abort,
                        ok: true,
                        message: format!("Git: abort finished ({repo_name})"),
                    })
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("abort failed: {e}")))
                    .await;
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::Abort,
                        ok: false,
                        message: format!("Git: abort failed ({repo_name})"),
                    })
                    .await;
            }
        }
    });
}

fn handle_abort_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("abort").unwrap_or("/abort");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("abort"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    let attempt_id = app
        .board
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    trigger_abort_conflicts(app, attempt_id, repo_id, &repo_name);
    Ok(())
}

fn handle_resolve_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("resolve").unwrap_or("/resolve [--repo R]");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("resolve"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    if app.board.selected_attempt_id.is_none() {
        return Err("no attempt selected".to_string());
    }
    if app.diff.repo_statuses.is_empty() {
        request_branch_status_refresh(app);
        return Err("no repo status loaded yet (run /status)".to_string());
    }

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
    app.diff.selected_repo_index = idx;
    app.ui.last_notice = Some(format!(
        "Selected repo: {}",
        app.diff.repo_statuses[idx].repo_name
    ));
    Ok(())
}

fn handle_rebase_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("rebase").unwrap_or("/rebase [--onto B]");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("rebase"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);
    let onto = parsed.get_value("--onto").map(ToString::to_string);
    let old = parsed.get_value("--old").map(ToString::to_string);

    let attempt_id = app
        .board
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
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

    if !begin_git_op(app, Some(repo_id), GitOpKind::Rebase, &repo_name) {
        return Ok(());
    }

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match rebase_task_attempt_http(&base_url, attempt_id, repo_id, old, onto).await {
            Ok(()) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!("Rebase started for {repo_name}.")))
                    .await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                }
                let _ = net_tx.send(NetEvent::DiffReconnect).await;
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::Rebase,
                        ok: true,
                        message: format!("Git: rebase finished ({repo_name})"),
                    })
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("rebase failed: {e}")))
                    .await;
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::Rebase,
                        ok: false,
                        message: format!("Git: rebase failed ({repo_name})"),
                    })
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_merge_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("merge").unwrap_or("/merge");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("merge"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);

    let attempt_id = app
        .board
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
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

    if !begin_git_op(app, Some(repo_id), GitOpKind::Merge, &repo_name) {
        return Ok(());
    }

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match merge_task_attempt_http(&base_url, attempt_id, repo_id).await {
            Ok(()) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!("Merged {repo_name}.")))
                    .await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                }
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::Merge,
                        ok: true,
                        message: format!("Git: merge finished ({repo_name})"),
                    })
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("merge failed: {e}")))
                    .await;
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind: GitOpKind::Merge,
                        ok: false,
                        message: format!("Git: merge failed ({repo_name})"),
                    })
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_push_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let help = crate::slash::help_syntax_for_command("push").unwrap_or("/push [--force]");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("push"), help)?;
    let repo_arg = parsed.get_value("--repo").map(ToString::to_string);
    let force = parsed.get_bool("--force");

    let attempt_id = app
        .board
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    let kind = if force {
        GitOpKind::ForcePush
    } else {
        GitOpKind::Push
    };
    if !begin_git_op(app, Some(repo_id), kind, &repo_name) {
        return Ok(());
    }

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        let result = if force {
            force_push_task_attempt_branch_http(&base_url, attempt_id, repo_id).await
        } else {
            push_task_attempt_branch_http(&base_url, attempt_id, repo_id).await
        };
        match result {
            Ok(()) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!(
                        "Pushed {repo_name}{}.",
                        if force { " (force)" } else { "" }
                    )))
                    .await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                }
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind,
                        ok: true,
                        message: format!(
                            "Git: push finished ({repo_name}{})",
                            if force { ", force" } else { "" }
                        ),
                    })
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("push failed: {e}")))
                    .await;
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind,
                        ok: false,
                        message: format!("Git: push failed ({repo_name})"),
                    })
                    .await;
            }
        }
    });
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

    let attempt_id = app
        .board
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
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

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
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
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
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

    let attempt_id = app
        .board
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    if !begin_git_op(app, Some(repo_id), GitOpKind::AttachPr, &repo_name) {
        return Ok(());
    }

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
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
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
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

    let attempt_id = app
        .board
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    if !begin_git_op(app, Some(repo_id), GitOpKind::PrComments, &repo_name) {
        return Ok(());
    }

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
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
    let attempt_id = app
        .board
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    let file_path = tokens[1].clone();

    tokio::spawn(async move {
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

pub(crate) fn resolve_repo_for_command(
    app: &mut AppState,
    repo_arg: Option<&str>,
) -> Result<(Uuid, String), String> {
    if app.diff.repo_statuses.is_empty() {
        request_branch_status_refresh(app);
        return Err("no repo status loaded yet (run /status)".to_string());
    }

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
        app.diff.selected_repo_index = idx;
    }

    let repo = app
        .diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .ok_or_else(|| "no repo selected".to_string())?;
    Ok((repo.repo_id, repo.repo_name.clone()))
}
