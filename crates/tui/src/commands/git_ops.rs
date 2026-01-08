use std::time::Instant;

use crate::{
    events::GitOpKind,
    state::{AppState, GitOpState},
};

pub(crate) fn begin_git_op(
    app: &mut AppState,
    repo_id: Option<uuid::Uuid>,
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
    repo_id: Option<uuid::Uuid>,
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
