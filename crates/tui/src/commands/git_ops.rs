use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::{
    commands::run_net_job,
    events::{GitOpKind, NetEvent, NetOpError},
    net::ops::branch_status_http,
    state::{AppState, GitOpState, JobKey, PendingExecHook},
};

pub(crate) fn request_branch_status_refresh(app: &mut AppState) {
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };
    if app.diff.repo_statuses.is_empty() {
        crate::ui::loading::start_with_default_delay(
            &mut app.diff.branch_status_loading_notice,
            true,
        );
    }

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
                        .send(
                            NetOpError::new("branch status", e)
                                .with_key(crate::state::UiMessageKey::BranchStatus)
                                .into_event(),
                        )
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
    if app.diff.repo_statuses.is_empty() {
        crate::ui::loading::start_with_default_delay(
            &mut app.diff.branch_status_loading_notice,
            true,
        );
    }

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
                        .send(
                            NetOpError::new("branch status", e)
                                .with_key(crate::state::UiMessageKey::BranchStatus)
                                .into_event(),
                        )
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
    let Some(pending) = app.exec.pending_branch_refresh.take() else {
        return;
    };

    let update = pending.on_exec_store_updated(&app.exec.exec_store);
    app.exec.pending_branch_refresh = update.next;
    if update.should_refresh {
        schedule_branch_status_refresh(app, Duration::ZERO);
    }
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
    crate::ui::toasts::warn_sticky(app, format!("Git: {}…{scope}", kind.label()));
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

    if ok {
        crate::ui::toasts::ok_medium(app, message);
    } else {
        crate::ui::toasts::err_medium(app, message);
    }
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
    let keep_for = crate::ui::constants::TOAST_SHORT;
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
