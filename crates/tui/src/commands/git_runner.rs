use std::{fmt::Display, future::Future};

use uuid::Uuid;

use crate::{
    commands::{begin_git_op, spawn_net_task},
    events::{GitOpKind, NetEvent, NetOpError},
    net::ops::branch_status_http,
    state::AppState,
};

pub(crate) struct GitOpOutcome {
    pub(crate) notice: Option<String>,
    pub(crate) refresh_branch_status: bool,
    pub(crate) diff_reconnect: bool,
    pub(crate) finished_message_ok: Option<String>,
    pub(crate) finished_message_err: Option<String>,
}

impl Default for GitOpOutcome {
    fn default() -> Self {
        Self {
            notice: None,
            refresh_branch_status: true,
            diff_reconnect: false,
            finished_message_ok: None,
            finished_message_err: None,
        }
    }
}

pub(crate) fn spawn_repo_git_op<F, Fut, E>(
    app: &mut AppState,
    attempt_id: Uuid,
    repo_id: Uuid,
    kind: GitOpKind,
    repo_name: &str,
    outcome: GitOpOutcome,
    op: F,
) where
    F: FnOnce(String) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), E>> + Send + 'static,
    E: Display + Send + 'static,
{
    if !begin_git_op(app, Some(repo_id), kind, repo_name) {
        return;
    }

    let repo_name = repo_name.to_string();
    spawn_net_task(app, move |base_url, net_tx| async move {
        match op(base_url.clone()).await {
            Ok(()) => {
                if let Some(notice) = outcome.notice {
                    let _ = net_tx.send(NetEvent::Notice(notice)).await;
                }
                if outcome.refresh_branch_status {
                    if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                        let _ = net_tx
                            .send(NetEvent::BranchStatusLoaded {
                                attempt_id,
                                statuses,
                            })
                            .await;
                    }
                }
                if outcome.diff_reconnect {
                    let _ = net_tx.send(NetEvent::DiffReconnect).await;
                }
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind,
                        ok: true,
                        message: outcome.finished_message_ok.unwrap_or_else(|| {
                            format!("Git: {} finished ({repo_name})", kind.label())
                        }),
                    })
                    .await;
            }
            Err(e) => {
                // Even on failure, the repo state may have changed (e.g. rebase left conflicts or
                // started an in-progress state). Refresh status so the UI reflects reality.
                if outcome.refresh_branch_status {
                    if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                        let _ = net_tx
                            .send(NetEvent::BranchStatusLoaded {
                                attempt_id,
                                statuses,
                            })
                            .await;
                    }
                }
                let op = match kind {
                    GitOpKind::Status => "git status",
                    GitOpKind::Merge => "git merge",
                    GitOpKind::Rebase => "git rebase",
                    GitOpKind::CreatePr => "git create pr",
                    GitOpKind::Abort => "git abort",
                    GitOpKind::Push => "git push",
                    GitOpKind::ForcePush => "git force push",
                    GitOpKind::AttachPr => "git attach pr",
                    GitOpKind::PrComments => "git pr comments",
                };
                let _ = net_tx
                    .send(
                        NetOpError::new(op, anyhow::anyhow!("{e}"))
                            .with_key(crate::state::UiMessageKey::GitOp)
                            .into_event(),
                    )
                    .await;
                let _ = net_tx
                    .send(NetEvent::GitOpFinished {
                        repo_id: Some(repo_id),
                        kind,
                        ok: false,
                        message: outcome.finished_message_err.unwrap_or_else(|| {
                            format!("Git: {} failed ({repo_name})", kind.label())
                        }),
                    })
                    .await;
            }
        }
    });
}
