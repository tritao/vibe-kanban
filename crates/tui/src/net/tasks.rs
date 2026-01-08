use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{events::NetEvent, net::api_client, state::AttemptRow};

pub(crate) async fn load_info_task(base_url: String, net_tx: mpsc::Sender<NetEvent>) {
    let url = api_client::url(&base_url, "/api/info");
    let client = match api_client::http_client() {
        Ok(c) => c,
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(crate::fmt::op_failed(
                    "http client init",
                    e,
                )))
                .await;
            return;
        }
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::InfoLoaded {
                    ok: false,
                    summary: crate::fmt::op_failed("fetch /api/info", e),
                })
                .await;
            return;
        }
    };

    let parsed = api_client::decode_api_response::<serde_json::Value>(resp).await;
    match parsed {
        Ok(api) => {
            let ok = api.is_success();
            let data = api.into_data();
            if let Some(info) = data.as_ref() {
                let profiles = crate::store::info::extract_executor_profiles(info);
                let _ = net_tx
                    .send(NetEvent::ExecutorProfilesLoaded {
                        available: profiles.available,
                        selected: profiles.selected,
                        profiles_executors: profiles.profiles_executors,
                    })
                    .await;
            }
            let summary = data
                .as_ref()
                .and_then(crate::store::info::summarize_info)
                .unwrap_or_else(|| "loaded /api/info".to_string());
            let _ = net_tx.send(NetEvent::InfoLoaded { ok, summary }).await;
        }
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::InfoLoaded {
                    ok: false,
                    summary: crate::fmt::op_failed("decode /api/info", e),
                })
                .await;
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct WorkspaceDto {
    id: Uuid,
    branch: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    setup_completed_at: Option<String>,
}

pub(crate) async fn load_attempts_task(
    base_url: String,
    task_id: Uuid,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let url = api_client::url(&base_url, &format!("/api/task-attempts?task_id={task_id}"));

    let client = match api_client::http_client() {
        Ok(c) => c,
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(crate::fmt::op_failed(
                    "http client init",
                    e,
                )))
                .await;
            return;
        }
    };

    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(crate::fmt::op_failed(
                    "fetch task attempts",
                    e,
                )))
                .await;
            return;
        }
    };

    let parsed = api_client::decode_api_response::<Vec<WorkspaceDto>>(resp).await;
    match parsed {
        Ok(api) => {
            if !api.is_success() {
                let _ = net_tx
                    .send(NetEvent::Error("failed to load task attempts".to_string()))
                    .await;
                return;
            }
            let attempts = api
                .into_data()
                .unwrap_or_default()
                .into_iter()
                .map(|w| AttemptRow {
                    id: w.id,
                    branch: w.branch,
                    created_at: w.created_at,
                    updated_at: w.updated_at,
                    setup_completed_at: w.setup_completed_at,
                })
                .collect();

            let _ = net_tx
                .send(NetEvent::AttemptsLoaded { task_id, attempts })
                .await;
        }
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(crate::fmt::op_failed(
                    "decode task attempts response",
                    e,
                )))
                .await;
        }
    }
}
