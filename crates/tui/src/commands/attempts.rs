use std::time::Duration;

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    events::NetEvent,
    net::ops::{
        create_task_attempt_http, latest_session_id_http, list_task_attempts_http,
        project_repositories_http, repo_branches_http,
    },
    state::ExecutorProfileSelection,
};

pub(crate) async fn ensure_session_id_for_message(
    base_url: &str,
    net_tx: &mpsc::Sender<NetEvent>,
    attempt_id: Option<Uuid>,
    task_id: Option<Uuid>,
    project_id: Option<Uuid>,
    executor_profile: Option<ExecutorProfileSelection>,
) -> Option<Uuid> {
    let attempt_id = ensure_attempt_id(
        base_url,
        net_tx,
        attempt_id,
        task_id,
        project_id,
        executor_profile,
    )
    .await?;

    // The workspace start can be async; poll briefly for a session to appear.
    for _ in 0..40 {
        match latest_session_id_http(base_url, attempt_id).await {
            Ok(Some(sid)) => return Some(sid),
            Ok(None) => tokio::time::sleep(Duration::from_millis(250)).await,
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(crate::fmt::op_failed(
                        "failed to load sessions",
                        e,
                    )))
                    .await;
                return None;
            }
        }
    }

    match latest_session_id_http(base_url, attempt_id).await {
        Ok(Some(sid)) => Some(sid),
        Ok(None) => {
            let _ = net_tx
                .send(NetEvent::Error(
                    "no session available for this attempt (workspace still starting?)".to_string(),
                ))
                .await;
            None
        }
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(crate::fmt::op_failed(
                    "failed to load sessions",
                    e,
                )))
                .await;
            None
        }
    }
}

pub(crate) async fn ensure_attempt_id_for_repo_ops(
    base_url: &str,
    net_tx: &mpsc::Sender<NetEvent>,
    attempt_id: Option<Uuid>,
    task_id: Option<Uuid>,
    project_id: Option<Uuid>,
    executor_profile: Option<ExecutorProfileSelection>,
) -> Option<Uuid> {
    ensure_attempt_id(
        base_url,
        net_tx,
        attempt_id,
        task_id,
        project_id,
        executor_profile,
    )
    .await
}

async fn ensure_attempt_id(
    base_url: &str,
    net_tx: &mpsc::Sender<NetEvent>,
    attempt_id: Option<Uuid>,
    task_id: Option<Uuid>,
    project_id: Option<Uuid>,
    executor_profile: Option<ExecutorProfileSelection>,
) -> Option<Uuid> {
    if let Some(id) = attempt_id {
        return Some(id);
    }

    let Some(task_id) = task_id else {
        let _ = net_tx
            .send(NetEvent::Error(
                "no task selected; cannot create attempt".to_string(),
            ))
            .await;
        return None;
    };

    let existing_attempts = match list_task_attempts_http(base_url, task_id).await {
        Ok(a) => a,
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(crate::fmt::op_failed(
                    "failed to load task attempts",
                    e,
                )))
                .await;
            return None;
        }
    };
    let _ = net_tx
        .send(NetEvent::AttemptsLoaded {
            task_id,
            attempts: existing_attempts.clone(),
        })
        .await;

    if let Some(first) = existing_attempts.first() {
        return Some(first.id);
    }

    let Some(project_id) = project_id else {
        let _ = net_tx
            .send(NetEvent::Error(
                "no project selected; cannot create attempt".to_string(),
            ))
            .await;
        return None;
    };
    let Some(executor_profile) = executor_profile else {
        let _ = net_tx
            .send(NetEvent::Error(
                "no executor selected yet; wait for /api/info".to_string(),
            ))
            .await;
        return None;
    };

    let repos = match project_repositories_http(base_url, project_id).await {
        Ok(r) => r,
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(crate::fmt::op_failed(
                    "failed to load project repositories",
                    e,
                )))
                .await;
            return None;
        }
    };
    if repos.is_empty() {
        let _ = net_tx
            .send(NetEvent::Error(
                "project has no repositories; add one first".to_string(),
            ))
            .await;
        return None;
    }

    let mut repo_inputs: Vec<(Uuid, String)> = Vec::with_capacity(repos.len());
    for repo in repos {
        let branches = repo_branches_http(base_url, repo.id)
            .await
            .unwrap_or_default();
        let target_branch = branches
            .iter()
            .find(|b| b.is_current && !b.is_remote)
            .or_else(|| branches.iter().find(|b| b.is_current))
            .map(|b| b.name.clone())
            .unwrap_or_else(|| "main".to_string());
        repo_inputs.push((repo.id, target_branch));
    }

    let created =
        match create_task_attempt_http(base_url, task_id, &executor_profile, repo_inputs).await {
            Ok(a) => a,
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(crate::fmt::op_failed(
                        "failed to create task attempt",
                        e,
                    )))
                    .await;
                return None;
            }
        };

    let _ = net_tx
        .send(NetEvent::AttemptsLoaded {
            task_id,
            attempts: vec![created.clone()],
        })
        .await;
    let _ = net_tx
        .send(NetEvent::Notice(format!(
            "Started attempt on branch {}.",
            created.branch
        )))
        .await;
    Some(created.id)
}
