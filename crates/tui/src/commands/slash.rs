use std::time::{Duration, Instant};

use ratatui::style::Color;
use uuid::Uuid;

use crate::ui::{trigger_diff_repo_action, DiffRepoAction};
use crate::events::{GitOpKind, NetEvent};
use crate::net::ops::{
    abort_conflicts_http, attach_pr_http, branch_status_http, create_pr_http, follow_up_http,
    force_push_task_attempt_branch_http, get_pr_comments_http, latest_session_id_http,
    merge_task_attempt_http, open_editor_http, push_task_attempt_branch_http,
    queue_follow_up_http, rebase_task_attempt_http, CreateGitHubPrRequest,
};
use crate::selection::{exec_list, find_task};
use crate::state::{AppState, Merge};
use crate::commands::open_url;
use super::git_ops::{begin_git_op, request_branch_status_refresh, set_toast};

pub(crate) fn submit_composer(app: &mut AppState) {
    let msg = app.ui.composer.buffer.trim_end().to_string();
    if msg.trim().is_empty() {
        app.ui.composer_active = false;
        app.ui.composer.clear();
        return;
    }

    app.ui.composer_active = false;
    app.ui.composer.clear();

    if crate::slash::composer_is_slash_mode(&msg) {
        submit_slash_command(app, &msg);
        return;
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
}

pub(crate) fn submit_slash_command(app: &mut AppState, raw: &str) {
    let cmdline = raw.trim_start().trim_start_matches('/');
    let tokens = match crate::cli_parse::tokenize_command_line(cmdline) {
        Ok(t) => t,
        Err(e) => {
            app.ui.last_error = Some(format!("invalid command: {e}"));
            return;
        }
    };

    if tokens.is_empty() {
        app.ui.last_error = Some("invalid command: empty".to_string());
        return;
    }

    match parse_slash_command(app, &tokens) {
        Ok(()) => {}
        Err(e) => {
            app.ui.last_error = Some(e);
        }
    }
}

fn parse_slash_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let cmd = crate::slash::canonical_command_name(tokens[0].as_str()).unwrap_or(tokens[0].as_str());
    match cmd {
        "help" => {
            app.ui.show_help = true;
            app.ui.last_notice = Some("Opened help. (Press Esc to close)".to_string());
            Ok(())
        }
        "status" => {
            trigger_diff_repo_action(app, DiffRepoAction::RefreshStatus);
            Ok(())
        }
        "resolve" => handle_resolve_command(app, tokens),
        "repo" => handle_repo_command(app, tokens.get(1).map(|s| s.as_str())),
        "rebase" => handle_rebase_command(app, tokens),
        "abort" => handle_abort_command(app, tokens),
        "merge" => handle_merge_command(app, tokens),
        "push" => handle_push_command(app, tokens),
        "pr" => handle_pr_command(app, tokens),
        "open" => handle_open_command(app, tokens),
        _ => Err(crate::slash::unknown_command_error(tokens[0].as_str())),
    }
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
    let mut repo_arg: Option<String> = None;
    let mut i = 1;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

    let attempt_id = app
        .board
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    trigger_abort_conflicts(app, attempt_id, repo_id, &repo_name);
    Ok(())
}

fn handle_resolve_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let mut repo_arg: Option<String> = None;
    let mut i = 1;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

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
            .is_some_and(|r| r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty());
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
    let mut repo_arg: Option<String> = None;
    let mut onto: Option<String> = None;
    let mut old: Option<String> = None;

    let mut i = 1;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            "--onto" => {
                i += 1;
                onto = tokens.get(i).cloned();
            }
            "--old" => {
                i += 1;
                old = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

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
    let mut repo_arg: Option<String> = None;
    let mut i = 1;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

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
    let mut repo_arg: Option<String> = None;
    let mut force = false;

    let mut i = 1;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            "--force" => force = true,
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

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
        return Err(
            crate::slash::usage_for_command("pr")
                .unwrap_or("usage: /pr <create|attach|comments|open>")
                .to_string(),
        );
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
    let mut repo_arg: Option<String> = None;
    let mut i = 2;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

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
    let mut repo_arg: Option<String> = None;
    let mut title: Option<String> = None;
    let mut body: Option<String> = None;
    let mut base: Option<String> = None;
    let mut draft: Option<bool> = None;
    let mut auto_desc = false;

    let mut i = 2;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            "--title" => {
                i += 1;
                title = tokens.get(i).cloned();
            }
            "--body" => {
                i += 1;
                body = tokens.get(i).cloned();
            }
            "--base" => {
                i += 1;
                base = tokens.get(i).cloned();
            }
            "--draft" => {
                draft = Some(true);
            }
            "--auto-desc" => {
                auto_desc = true;
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

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

    let draft = draft.or(Some(false));

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
    let mut repo_arg: Option<String> = None;
    let mut i = 2;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

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
    let mut repo_arg: Option<String> = None;
    let mut i = 2;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

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
        return Err(
            crate::slash::usage_for_command("open")
                .unwrap_or("usage: /open <file_path>")
                .to_string(),
        );
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
