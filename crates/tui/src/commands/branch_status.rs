use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::{
    commands::run_net_job,
    events::NetEvent,
    jobs::job_running,
    net::ops::branch_status_http,
    state::{AppState, JobKey, PendingExecHook, UiMessageKey},
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
                            crate::events::NetOpError::new("branch status", e)
                                .with_key(UiMessageKey::BranchStatus)
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
                            crate::events::NetOpError::new("branch status", e)
                                .with_key(UiMessageKey::BranchStatus)
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

pub(crate) fn tick_branch_status_auto_refresh(app: &mut AppState, now: Instant) -> bool {
    // While the diff pane is focused, keep repo status reasonably fresh so action enablement and
    // conflict indicators remain accurate without requiring manual `S`.
    if app.ui.focus == crate::state::FocusPane::Diff
        && app.board.selected_attempt_id.is_some()
        && !app.diff.repo_statuses.is_empty()
        && !job_running(app, JobKey::BranchStatus)
        && !job_running(app, JobKey::BranchStatusAuto)
    {
        let stale = app
            .diff
            .branch_status_loaded_at
            .map(|t| now.saturating_duration_since(t))
            .unwrap_or(crate::ui::constants::BRANCH_STATUS_AUTO_REFRESH_INTERVAL);
        let due = app
            .diff
            .branch_status_auto_next_at
            .map(|t| now >= t)
            .unwrap_or(stale >= crate::ui::constants::BRANCH_STATUS_AUTO_REFRESH_INTERVAL);
        if due {
            app.diff.branch_status_auto_next_at =
                Some(now + crate::ui::constants::BRANCH_STATUS_AUTO_REFRESH_INTERVAL);
            schedule_branch_status_refresh_debounced(app, Duration::ZERO);
            return true;
        }
    }
    false
}

pub(crate) fn tick_branch_status_loading_notice(app: &mut AppState, now: Instant) -> bool {
    // If repo status isn't loaded yet, show a delayed "Loading repos…" notice while the refresh
    // job runs (prevents flicker on fast responses).
    let branch_running =
        job_running(app, JobKey::BranchStatus) || job_running(app, JobKey::BranchStatusAuto);
    let placeholder = crate::ui::loading_placeholders::tick_notice_placeholder(
        now,
        &mut app.diff.branch_status_loading_notice,
        branch_running,
        app.diff.repo_statuses.is_empty(),
    );
    if placeholder.show_notice {
        app.ui
            .set_notice(crate::ui::messages::notices::LOADING_REPOS);
    }
    placeholder.dirty || placeholder.show_notice
}
