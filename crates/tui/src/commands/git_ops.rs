use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::{
    commands::run_net_job,
    events::{GitOpKind, NetEvent},
    net::ops::branch_status_http,
    selection::exec_list,
    state::{AppState, GitOpState, JobKey, PendingExecHook},
};

pub(crate) fn request_branch_status_refresh(app: &mut AppState) {
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };

    run_net_job(
        app,
        JobKey::BranchStatus,
        move |base_url, net_tx| async move {
            match branch_status_http(&base_url, attempt_id).await {
                Ok(statuses) => {
                    let _ = net_tx
                        .send(NetEvent::BranchStatusLoaded {
                            attempt_id,
                            statuses,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("branch status failed: {e}")))
                        .await;
                }
            }
        },
    );
}

pub(crate) fn schedule_branch_status_refresh(app: &mut AppState, delay: Duration) {
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };

    run_net_job(
        app,
        JobKey::BranchStatusAuto,
        move |base_url, net_tx| async move {
            tokio::time::sleep(delay).await;
            match branch_status_http(&base_url, attempt_id).await {
                Ok(statuses) => {
                    let _ = net_tx
                        .send(NetEvent::BranchStatusLoaded {
                            attempt_id,
                            statuses,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("branch status failed: {e}")))
                        .await;
                }
            }
        },
    );
}

pub(crate) fn schedule_branch_status_refresh_debounced(app: &mut AppState, delay: Duration) {
    const MIN_REFRESH_AGE: Duration = Duration::from_secs(8);
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };
    let now = Instant::now();
    if app.diff.branch_status_loaded_attempt_id == Some(attempt_id)
        && app
            .diff
            .branch_status_loaded_at
            .is_some_and(|t| now.saturating_duration_since(t) < MIN_REFRESH_AGE)
    {
        return;
    }
    schedule_branch_status_refresh(app, delay);
}

pub(crate) fn arm_branch_status_refresh_for_exec(app: &mut AppState, exec_id: Uuid) {
    app.exec.pending_branch_refresh = Some(PendingExecHook::for_exec(exec_id));
}

pub(crate) fn arm_branch_status_refresh_after_next_exec(
    app: &mut AppState,
    prev_exec_id: Option<Uuid>,
) {
    app.exec.pending_branch_refresh = Some(PendingExecHook::after_next_exec(prev_exec_id));
}

pub(crate) fn clear_pending_branch_status_refresh(app: &mut AppState) {
    app.exec.pending_branch_refresh = None;
}

pub(crate) fn on_exec_store_updated_for_branch_refresh(app: &mut AppState) {
    // Single place to maintain the pending-refresh state machine:
    // - If waiting for a new exec: bind to newest exec once it differs from prev.
    // - If bound to an exec: refresh when it is no longer running, then clear.
    let Some(mut pending) = app.exec.pending_branch_refresh else {
        return;
    };

    if pending.wait_new_exec {
        let mut execs = exec_list(&app.exec.exec_store);
        execs.sort_by_key(|e| e.created_at.clone().unwrap_or_default());
        if let Some(latest) = execs.last().map(|e| e.id) {
            if pending.prev_exec_id != Some(latest) {
                pending.exec_id = Some(latest);
                pending.prev_exec_id = None;
                pending.wait_new_exec = false;
            }
        }
    }

    let Some(exec_id) = pending.exec_id else {
        app.exec.pending_branch_refresh = Some(pending);
        return;
    };

    let execs = exec_list(&app.exec.exec_store);
    let status = execs
        .iter()
        .find(|e| e.id == exec_id)
        .and_then(|e| e.status.as_deref())
        .unwrap_or("unknown");
    if status == "running" {
        app.exec.pending_branch_refresh = Some(pending);
        return;
    }

    schedule_branch_status_refresh(app, Duration::from_millis(0));
    app.exec.pending_branch_refresh = None;
}

pub(crate) fn request_diff_reconnect(app: &mut AppState) {
    let next = *app.diff_reconnect_tx.borrow() + 1;
    let _ = app.diff_reconnect_tx.send(next);
}

pub(crate) fn begin_git_op(
    app: &mut AppState,
    repo_id: Option<Uuid>,
    kind: GitOpKind,
    repo_name: &str,
) -> bool {
    let now = Instant::now();

    let already_running = match repo_id {
        Some(id) => app
            .diff
            .git_ops
            .get(&id)
            .is_some_and(|s| s.finished_at.is_none()),
        None => app
            .diff
            .git_op_global
            .as_ref()
            .is_some_and(|s| s.finished_at.is_none()),
    };
    if already_running {
        app.ui
            .set_notice(format!("Git: {} already running.", kind.label()));
        return false;
    }

    let state = GitOpState {
        kind,
        started_at: now,
        finished_at: None,
        ok: None,
    };
    match repo_id {
        Some(id) => {
            app.diff.git_ops.insert(id, state);
        }
        None => {
            app.diff.git_op_global = Some(state);
        }
    }

    let scope = if repo_name.is_empty() {
        "".to_string()
    } else {
        format!(" ({repo_name})")
    };
    app.ui.set_toast(
        format!("Git: {}…{scope}", kind.label()),
        crate::ui::palette::toast_warn(),
        None,
    );
    true
}

pub(crate) fn finish_git_op(
    app: &mut AppState,
    repo_id: Option<Uuid>,
    kind: GitOpKind,
    ok: bool,
    message: String,
) {
    let now = Instant::now();
    let finished = match repo_id {
        Some(id) => app.diff.git_ops.get_mut(&id),
        None => app.diff.git_op_global.as_mut(),
    };
    if let Some(state) = finished {
        if state.kind == kind && state.finished_at.is_none() {
            state.finished_at = Some(now);
            state.ok = Some(ok);
        }
    }

    app.ui.set_toast(
        message,
        if ok {
            crate::ui::palette::toast_ok()
        } else {
            crate::ui::palette::toast_err()
        },
        Some(Duration::from_secs(3)),
    );
}

pub(crate) fn update_git_activity_indicators(app: &mut AppState, now: Instant) -> bool {
    let mut dirty = false;

    // Animate while any op is running (spinner/elapsed).
    let any_running_repo = app.diff.git_ops.values().any(|s| s.finished_at.is_none());
    let any_running_global = app
        .diff
        .git_op_global
        .as_ref()
        .is_some_and(|s| s.finished_at.is_none());
    if any_running_repo || any_running_global {
        dirty = true;
    }

    // Drop completed repo ops after a short grace period (for ✓ feedback).
    let keep_for = Duration::from_secs(2);
    let before = app.diff.git_ops.len();
    app.diff.git_ops.retain(|_, s| {
        s.finished_at.is_none() || now.saturating_duration_since(s.finished_at.unwrap()) < keep_for
    });
    if app.diff.git_ops.len() != before {
        dirty = true;
    }

    if let Some(s) = app.diff.git_op_global.as_ref().and_then(|s| s.finished_at) {
        if now.saturating_duration_since(s) >= keep_for {
            app.diff.git_op_global = None;
            dirty = true;
        }
    }

    // Expire toast (only those with an expiry; running toasts are sticky).
    if let Some(t) = app.ui.toast.as_ref() {
        if let Some(exp) = t.expires_at {
            if now >= exp {
                app.ui.toast = None;
                dirty = true;
            }
        }
    }

    dirty
}
