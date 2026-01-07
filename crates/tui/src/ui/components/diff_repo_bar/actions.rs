use std::time::Duration;

use ratatui::style::Color;

use super::{DiffRepoAction, shared};
use crate::{
    commands::{
        begin_git_op, open_url, request_branch_status_refresh, resolve_repo_for_command,
        trigger_abort_conflicts,
    },
    events::{GitOpKind, NetEvent},
    layout::{compute_main_layout, current_terminal_rect},
    net::ops::{
        CreateGitHubPrRequest, branch_status_http, create_pr_http, merge_task_attempt_http,
        open_editor_http, rebase_task_attempt_http,
    },
    selection::find_task,
    state::{AppState, FocusPane, Merge, build_resolve_conflicts_instructions},
};

fn toast(app: &mut AppState, message: impl Into<String>, color: Color, seconds: u64) {
    app.ui
        .set_toast(message, color, Some(Duration::from_secs(seconds)));
}

fn toast_short(app: &mut AppState, message: impl Into<String>, color: Color) {
    app.ui
        .set_toast(message, color, Some(crate::ui::constants::TOAST_SHORT));
}

fn toast_medium(app: &mut AppState, message: impl Into<String>, color: Color) {
    app.ui
        .set_toast(message, color, Some(crate::ui::constants::TOAST_MEDIUM));
}

fn ensure_attempt_selected(app: &mut AppState, context: &str) -> bool {
    if app.board.selected_attempt_id.is_some() {
        return true;
    }
    toast_short(
        app,
        format!("{context}: no attempt selected"),
        crate::ui::palette::toast_err(),
    );
    false
}

fn ensure_repo_status_loaded(app: &mut AppState, context: &str) -> bool {
    if !app.diff.repo_statuses.is_empty() {
        return true;
    }
    request_branch_status_refresh(app);
    toast_short(
        app,
        format!("{context}: loading repo status…"),
        crate::ui::palette::toast_warn(),
    );
    false
}

pub(crate) fn trigger_diff_repo_action(app: &mut AppState, action: DiffRepoAction) {
    match action {
        DiffRepoAction::ResolveConflicts => {
            if !ensure_attempt_selected(app, "Conflicts") {
                return;
            };
            if !ensure_repo_status_loaded(app, "Conflicts") {
                return;
            }

            let Some(idx) = shared::repo_index_with_conflicts(app) else {
                toast_short(
                    app,
                    "Conflicts: none detected",
                    crate::ui::palette::toast_ok(),
                );
                return;
            };
            crate::selection_hooks::set_selected_repo_index(app, idx);
            let repo = match app.diff.repo_statuses.get(idx) {
                Some(r) => r,
                None => return,
            };

            let attempt_branch = shared::selected_attempt_branch(app);
            let instructions = build_resolve_conflicts_instructions(
                Some(&attempt_branch),
                Some(&repo.status.target_branch_name),
                &repo.status.conflicted_files,
                repo.status.conflict_op,
                Some(&repo.repo_name),
            );

            app.ui.focus_execution();
            app.ui.composer_active = true;
            app.ui.composer_suggest_index = 0;
            app.ui.refresh_branch_status_after_send = true;
            app.ui.composer.buffer = instructions;
            app.ui.composer.set_end();
            let layout = compute_main_layout(current_terminal_rect(), FocusPane::Execution);
            let area = layout.exec_input;
            let inner_w = area.width.saturating_sub(2) as usize;
            let inner_h = area.height.saturating_sub(2) as usize;
            let prefix_w = crate::text::display_width("  ");
            let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);
            app.ui
                .composer
                .ensure_cursor_visible(content_w, inner_h.max(1));
            toast_short(
                app,
                format!("Conflicts: drafted resolution request ({})", repo.repo_name),
                crate::ui::palette::toast_info(),
            );
        }
        DiffRepoAction::OpenConflict => {
            let Some(attempt_id) = app.board.selected_attempt_id else {
                toast_short(
                    app,
                    "Open: no attempt selected",
                    crate::ui::palette::toast_err(),
                );
                return;
            };
            if !ensure_repo_status_loaded(app, "Open") {
                return;
            }

            let Some(idx) = shared::repo_index_with_conflicts(app) else {
                toast_short(
                    app,
                    "Open: no conflicts detected",
                    crate::ui::palette::toast_ok(),
                );
                return;
            };
            crate::selection_hooks::set_selected_repo_index(app, idx);
            let Some(repo) = app.diff.repo_statuses.get(idx) else {
                return;
            };
            let Some(first) = repo.status.conflicted_files.first().cloned() else {
                toast_short(
                    app,
                    "Open: no conflicted files listed",
                    crate::ui::palette::toast_warn(),
                );
                return;
            };

            let repo_name = repo.repo_name.clone();
            crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
                match open_editor_http(&base_url, attempt_id, Some(first.clone())).await {
                    Ok(url) => {
                        let msg = match url {
                            Some(url) => {
                                format!("Opened conflict file ({repo_name}): {first} ({url})")
                            }
                            None => format!("Opened conflict file ({repo_name}): {first}"),
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
        }
        DiffRepoAction::OpenPr => {
            let Some(repo) = shared::selected_repo_status(app) else {
                toast_short(
                    app,
                    "PR: no repo selected",
                    crate::ui::palette::toast_warn(),
                );
                return;
            };
            let Some(pr) = repo.status.merges.iter().find_map(|m| match m {
                Merge::Pr(pr) => Some(&pr.pr_info),
                _ => None,
            }) else {
                toast_short(app, "PR: none attached", crate::ui::palette::toast_warn());
                return;
            };

            match open_url(&pr.url) {
                Ok(()) => {
                    toast_short(
                        app,
                        format!("PR: opened (PR#{})", pr.number),
                        crate::ui::palette::toast_ok(),
                    );
                }
                Err(e) => {
                    app.ui.set_notice(format!("PR URL: {}", pr.url));
                    toast_medium(
                        app,
                        format!("PR: failed to open ({e})"),
                        crate::ui::palette::toast_err(),
                    );
                }
            }
        }
        DiffRepoAction::AbortConflicts => {
            let Some(attempt_id) = app.board.selected_attempt_id else {
                toast_short(
                    app,
                    "Abort: no attempt selected",
                    crate::ui::palette::toast_err(),
                );
                return;
            };
            if !ensure_repo_status_loaded(app, "Abort") {
                return;
            }

            let Some(idx) = shared::repo_index_with_conflicts(app) else {
                toast_short(
                    app,
                    "Abort: no conflicts detected",
                    crate::ui::palette::toast_ok(),
                );
                return;
            };
            crate::selection_hooks::set_selected_repo_index(app, idx);
            let Some(repo) = app.diff.repo_statuses.get(idx) else {
                return;
            };
            let repo_id = repo.repo_id;
            let repo_name = repo.repo_name.clone();
            trigger_abort_conflicts(app, attempt_id, repo_id, &repo_name);
        }
        DiffRepoAction::RefreshStatus => {
            let repo = shared::selected_repo_status(app);
            let (repo_id, repo_name) = match repo {
                Some(r) => (Some(r.repo_id), r.repo_name.clone()),
                None => (None, String::new()),
            };

            if !begin_git_op(app, repo_id, GitOpKind::Status, &repo_name) {
                return;
            }

            let attempt_id = app.board.selected_attempt_id;
            let task_id = app.board.selected_task_id;
            let project_id = app.board.selected_project_id;
            let executor_profile = app.ui.selected_executor_profile.clone();
            crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
                let attempt_id = match crate::commands::ensure_attempt_id_for_repo_ops(
                    &base_url,
                    &net_tx,
                    attempt_id,
                    task_id,
                    project_id,
                    executor_profile,
                )
                .await
                {
                    Some(id) => id,
                    None => {
                        let _ = net_tx
                            .send(NetEvent::GitOpFinished {
                                repo_id,
                                kind: GitOpKind::Status,
                                ok: false,
                                message: "Git: status failed".to_string(),
                            })
                            .await;
                        return;
                    }
                };

                match branch_status_http(&base_url, attempt_id).await {
                    Ok(statuses) => {
                        let _ = net_tx
                            .send(NetEvent::BranchStatusLoaded {
                                attempt_id,
                                statuses,
                            })
                            .await;
                        let _ = net_tx
                            .send(NetEvent::GitOpFinished {
                                repo_id,
                                kind: GitOpKind::Status,
                                ok: true,
                                message: if repo_name.is_empty() {
                                    "Git: status updated".to_string()
                                } else {
                                    format!("Git: status updated ({repo_name})")
                                },
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = net_tx
                            .send(NetEvent::Error(crate::fmt::op_failed("branch status", e)))
                            .await;
                        let _ = net_tx
                            .send(NetEvent::GitOpFinished {
                                repo_id,
                                kind: GitOpKind::Status,
                                ok: false,
                                message: "Git: status failed".to_string(),
                            })
                            .await;
                    }
                }
            });
        }
        DiffRepoAction::Merge => {
            let Ok((repo_id, repo_name)) = resolve_repo_for_command(app, None) else {
                return;
            };
            let Some(attempt_id) = app.board.selected_attempt_id else {
                return;
            };
            let Some(r) = app.diff.repo_statuses.iter().find(|r| r.repo_id == repo_id) else {
                return;
            };
            if r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty() {
                toast(
                    app,
                    "Merge: conflicts in progress (resolve/abort first)",
                    crate::ui::palette::toast_warn(),
                    2,
                );
                return;
            }
            if r.status.commits_ahead.unwrap_or(0) == 0 {
                toast(
                    app,
                    "Merge: nothing to merge (up to date)",
                    crate::ui::palette::toast_ok(),
                    2,
                );
                return;
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
                move |base_url| async move {
                    merge_task_attempt_http(&base_url, attempt_id, repo_id).await
                },
            );
        }
        DiffRepoAction::Rebase => {
            let Ok((repo_id, repo_name)) = resolve_repo_for_command(app, None) else {
                return;
            };
            let Some(attempt_id) = app.board.selected_attempt_id else {
                return;
            };
            let Some(r) = app.diff.repo_statuses.iter().find(|r| r.repo_id == repo_id) else {
                return;
            };
            if r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty() {
                toast(
                    app,
                    "Rebase: conflicts in progress (resolve/abort first)",
                    crate::ui::palette::toast_warn(),
                    2,
                );
                return;
            }
            if r.status.commits_behind.unwrap_or(0) == 0 {
                toast(
                    app,
                    "Rebase: already up to date",
                    crate::ui::palette::toast_ok(),
                    2,
                );
                return;
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
                    rebase_task_attempt_http(&base_url, attempt_id, repo_id, None, None).await
                },
            );
        }
        DiffRepoAction::CreatePr => {
            let Ok((repo_id, repo_name)) = resolve_repo_for_command(app, None) else {
                return;
            };
            let Some(attempt_id) = app.board.selected_attempt_id else {
                return;
            };
            let Some(r) = app.diff.repo_statuses.iter().find(|r| r.repo_id == repo_id) else {
                return;
            };
            if r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty() {
                toast(
                    app,
                    "PR: conflicts in progress (resolve/abort first)",
                    crate::ui::palette::toast_warn(),
                    2,
                );
                return;
            }
            if r.status.commits_ahead.unwrap_or(0) == 0 {
                toast(
                    app,
                    "PR: no changes to open (up to date)",
                    crate::ui::palette::toast_ok(),
                    2,
                );
                return;
            }
            let pr_open = r.status.merges.iter().find_map(|m| match m {
                Merge::Pr(pr) => Some(pr.pr_info.number),
                _ => None,
            });
            if let Some(n) = pr_open {
                toast(
                    app,
                    format!("PR: already exists (PR#{n})"),
                    crate::ui::palette::toast_ok(),
                    2,
                );
                return;
            }
            if !begin_git_op(app, Some(repo_id), GitOpKind::CreatePr, &repo_name) {
                return;
            }
            let title = app
                .board
                .selected_task_id
                .and_then(|id| find_task(&app.board.tasks_store, id).map(|t| t.title))
                .unwrap_or_else(|| "Vibe Kanban PR".to_string());
            crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
                match create_pr_http(
                    &base_url,
                    attempt_id,
                    CreateGitHubPrRequest {
                        title,
                        body: None,
                        target_branch: None,
                        draft: Some(false),
                        repo_id,
                        auto_generate_description: false,
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
        }
    }
}
