use super::super::{
    context::{require_selected_attempt_id, resolve_repo_for_command},
    git_ops::begin_git_op,
};
use crate::{
    commands::open_url,
    events::{GitOpKind, NetEvent},
    net::ops::{
        CreateGitHubPrRequest, attach_pr_http, branch_status_http, create_pr_http,
        get_pr_comments_http,
    },
    selection::find_task,
    state::AppState,
    store::git_status::RepoStatuses,
};

pub(super) fn handle_pr_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
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
    let pr = crate::store::git_status::RepoStatusRef::new(repo).pr_info();
    let Some(pr) = pr else {
        return Err("no PR attached for selected repo".to_string());
    };
    if pr.url.trim().is_empty() {
        return Err("PR has no URL".to_string());
    }

    match open_url(&pr.url) {
        Ok(()) => {
            crate::ui::toasts::ok_short(app, format!("PR: opened (PR#{}, {repo_name})", pr.number));
            Ok(())
        }
        Err(e) => {
            app.ui.set_notice(format!("PR URL: {}", pr.url));
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

    if let Some(r) = RepoStatuses::new(&app.diff.repo_statuses).get_by_id(repo_id) {
        if r.has_conflicts() {
            crate::ui::toasts::warn_short(app, "PR: conflicts in progress (resolve/abort first)");
            return Ok(());
        }
        if r.commits_ahead() == 0 {
            crate::ui::toasts::ok_short(app, "PR: no changes to open (up to date)");
            return Ok(());
        }
        if let Some(n) = r.pr_number() {
            crate::ui::toasts::ok_short(app, format!("PR: already exists (PR#{n})"));
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
                    .send(NetEvent::ErrorKey {
                        key: crate::state::UiMessageKey::PullRequestOp,
                        message: format!("pr create failed: {e}"),
                    })
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
                    .send(NetEvent::ErrorKey {
                        key: crate::state::UiMessageKey::PullRequestOp,
                        message: format!("pr attach failed: {e}"),
                    })
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
                    .send(NetEvent::ErrorKey {
                        key: crate::state::UiMessageKey::PullRequestOp,
                        message: format!("pr comments failed: {e}"),
                    })
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
