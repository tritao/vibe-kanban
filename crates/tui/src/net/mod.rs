use anyhow::Context;
use tokio::sync::mpsc;
use utils::port_file::read_port_file;
use uuid::Uuid;

use crate::{Args, events::NetEvent, state::AttemptRow};

pub(crate) mod api_client;
pub(crate) mod ops;
pub(crate) mod streams;

pub(crate) use streams::{
    diff_stream_task, exec_stream_task, logs_stream_task, projects_stream_task, tasks_stream_task,
};

pub(crate) async fn resolve_backend_url(args: &Args) -> anyhow::Result<String> {
    if let Some(url) = args.backend_url.as_ref().filter(|s| !s.trim().is_empty()) {
        return Ok(url.trim_end_matches('/').to_string());
    }

    let host = args
        .host
        .clone()
        .or_else(|| std::env::var("HOST").ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let port = if let Some(p) = args.port {
        p
    } else if let Ok(port_str) = std::env::var("BACKEND_PORT").or_else(|_| std::env::var("PORT")) {
        port_str.parse::<u16>().context("invalid port value")?
    } else {
        match read_port_file("vibe-kanban").await {
            Ok(port) => port,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let port_path = std::env::temp_dir()
                    .join("vibe-kanban")
                    .join("vibe-kanban.port");
                return Err(anyhow::anyhow!(
                    "Could not find backend port. Start the backend (e.g. `pnpm run dev`), or pass `--backend-url http://127.0.0.1:PORT`, or set `BACKEND_PORT`.\nMissing port file: {}",
                    port_path.display()
                ));
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to read backend port file: {e} (set `BACKEND_PORT` or pass `--backend-url`)"
                ));
            }
        }
    };

    Ok(format!("http://{}:{}", host, port))
}

pub(crate) async fn load_info_task(base_url: String, net_tx: mpsc::Sender<NetEvent>) {
    let url = crate::net::api_client::url(&base_url, "/api/info");
    let client = match crate::net::api_client::http_client() {
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

    let parsed = crate::net::api_client::decode_api_response::<serde_json::Value>(resp).await;
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
    let url =
        crate::net::api_client::url(&base_url, &format!("/api/task-attempts?task_id={task_id}"));

    let client = match crate::net::api_client::http_client() {
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

    let parsed = crate::net::api_client::decode_api_response::<Vec<WorkspaceDto>>(resp).await;
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
