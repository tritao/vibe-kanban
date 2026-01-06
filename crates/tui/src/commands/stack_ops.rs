use uuid::Uuid;

use crate::{
    events::NetEvent,
    jobs::replace_job,
    net::ops::{
        stack_enable_http, stack_pop_http, stack_push_http, stack_redo_http, stack_status_http,
        stack_undo_http,
    },
    state::{AppState, JobKey},
};

pub(crate) fn request_stack_status_refresh(app: &mut AppState) {
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };
    let Some(repo) = app.diff.repo_statuses.get(app.diff.selected_repo_index) else {
        return;
    };
    let repo_id = repo.repo_id;

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::StackStatus,
        tokio::spawn(async move {
            match stack_status_http(&base_url, attempt_id, repo_id).await {
                Ok(status) => {
                    let _ = net_tx
                        .send(NetEvent::StackStatusLoaded { repo_id, status })
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("stack status failed: {e}")))
                        .await;
                }
            }
        }),
    );
}

pub(crate) fn trigger_stack_enable(app: &mut AppState, attempt_id: Uuid, repo_id: Uuid) {
    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::StackStatus,
        tokio::spawn(async move {
            match stack_enable_http(&base_url, attempt_id, repo_id).await {
                Ok(status) => {
                    let _ = net_tx
                        .send(NetEvent::StackStatusLoaded { repo_id, status })
                        .await;
                    let _ = net_tx
                        .send(NetEvent::Notice("Stack enabled.".to_string()))
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("stack enable failed: {e}")))
                        .await;
                }
            }
        }),
    );
}

pub(crate) fn trigger_stack_push(app: &mut AppState, attempt_id: Uuid, repo_id: Uuid) {
    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::StackStatus,
        tokio::spawn(async move {
            match stack_push_http(&base_url, attempt_id, repo_id).await {
                Ok(status) => {
                    let _ = net_tx
                        .send(NetEvent::StackStatusLoaded { repo_id, status })
                        .await;
                    let _ = net_tx
                        .send(NetEvent::Notice("Stack: push ok.".to_string()))
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("stack push failed: {e}")))
                        .await;
                }
            }
        }),
    );
}

pub(crate) fn trigger_stack_pop(app: &mut AppState, attempt_id: Uuid, repo_id: Uuid) {
    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::StackStatus,
        tokio::spawn(async move {
            match stack_pop_http(&base_url, attempt_id, repo_id).await {
                Ok(status) => {
                    let _ = net_tx
                        .send(NetEvent::StackStatusLoaded { repo_id, status })
                        .await;
                    let _ = net_tx
                        .send(NetEvent::Notice("Stack: pop ok.".to_string()))
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("stack pop failed: {e}")))
                        .await;
                }
            }
        }),
    );
}

pub(crate) fn trigger_stack_undo(app: &mut AppState, attempt_id: Uuid, repo_id: Uuid) {
    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::StackStatus,
        tokio::spawn(async move {
            match stack_undo_http(&base_url, attempt_id, repo_id).await {
                Ok(status) => {
                    let _ = net_tx
                        .send(NetEvent::StackStatusLoaded { repo_id, status })
                        .await;
                    let _ = net_tx
                        .send(NetEvent::Notice("Stack: undo ok.".to_string()))
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("stack undo failed: {e}")))
                        .await;
                }
            }
        }),
    );
}

pub(crate) fn trigger_stack_redo(app: &mut AppState, attempt_id: Uuid, repo_id: Uuid) {
    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::StackStatus,
        tokio::spawn(async move {
            match stack_redo_http(&base_url, attempt_id, repo_id).await {
                Ok(status) => {
                    let _ = net_tx
                        .send(NetEvent::StackStatusLoaded { repo_id, status })
                        .await;
                    let _ = net_tx
                        .send(NetEvent::Notice("Stack: redo ok.".to_string()))
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("stack redo failed: {e}")))
                        .await;
                }
            }
        }),
    );
}
