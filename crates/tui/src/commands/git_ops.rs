use std::time::{Duration, Instant};

use ratatui::style::Color;
use uuid::Uuid;

use crate::events::{GitOpKind, NetEvent};
use crate::net::ops::branch_status_http;
use crate::state::{AppState, GitOpState, ToastState};

pub(crate) fn request_branch_status_refresh(app: &mut AppState) {
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match branch_status_http(&base_url, attempt_id).await {
            Ok(statuses) => {
                let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("branch status failed: {e}")))
                    .await;
            }
        }
    });
}

pub(crate) fn request_diff_reconnect(app: &mut AppState) {
    let next = *app.diff_reconnect_tx.borrow() + 1;
    let _ = app.diff_reconnect_tx.send(next);
}

pub(crate) fn set_toast(
    app: &mut AppState,
    message: String,
    color: Color,
    expires_at: Option<Instant>,
) {
    app.ui.toast = Some(ToastState {
        message,
        color,
        expires_at,
    });
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
        app.ui.last_notice = Some(format!("Git: {} already running.", kind.label()));
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
    set_toast(
        app,
        format!("Git: {}…{scope}", kind.label()),
        Color::Yellow,
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

    set_toast(
        app,
        message,
        if ok { Color::Green } else { Color::Red },
        Some(now + Duration::from_secs(3)),
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
