use anyhow::Context;
use tokio::sync::mpsc;
use utils::{port_file::read_port_file, response::ApiResponse};
use uuid::Uuid;

use crate::{
    Args,
    events::NetEvent,
    state::{AttemptRow, ExecutorProfileSelection},
};

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
    let url = format!("{}/api/info", base_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(format!("reqwest init: {e}")))
                .await;
            return;
        }
    };

    let res = client.get(&url).send().await;
    match res {
        Ok(r) => {
            let parsed = r.json::<ApiResponse<serde_json::Value>>().await;
            match parsed {
                Ok(api) => {
                    let ok = api.is_success();
                    let data = api.into_data();
                    if let Some(info) = data.as_ref() {
                        let (available, selected, profiles_executors) =
                            extract_executor_profiles(info);
                        let _ = net_tx
                            .send(NetEvent::ExecutorProfilesLoaded {
                                available,
                                selected,
                                profiles_executors,
                            })
                            .await;
                    }
                    let summary = data
                        .and_then(|d| summarize_info(&d))
                        .unwrap_or_else(|| "loaded /api/info".to_string());
                    let _ = net_tx.send(NetEvent::InfoLoaded { ok, summary }).await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::InfoLoaded {
                            ok: false,
                            summary: format!("failed to parse /api/info: {e}"),
                        })
                        .await;
                }
            }
        }
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::InfoLoaded {
                    ok: false,
                    summary: format!("failed to fetch /api/info: {e}"),
                })
                .await;
        }
    }
}

fn extract_executor_profiles(
    info: &serde_json::Value,
) -> (
    Vec<String>,
    Option<ExecutorProfileSelection>,
    serde_json::Value,
) {
    let available = info
        .get("executors")
        .and_then(|v| v.as_object())
        .map(|o| {
            let mut keys: Vec<String> = o.keys().cloned().collect();
            keys.sort();
            keys
        })
        .unwrap_or_default();

    let profiles_executors = info
        .get("executors")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    let selected = info
        .get("config")
        .and_then(|c| c.get("executor_profile"))
        .and_then(|p| serde_json::from_value::<ExecutorProfileSelection>(p.clone()).ok());

    (available, selected, profiles_executors)
}

fn summarize_info(info: &serde_json::Value) -> Option<String> {
    let env = info.get("environment")?;
    let os_type = env.get("os_type")?.as_str().unwrap_or("unknown");
    let os_arch = env
        .get("os_architecture")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let login_status = info.get("login_status")?;

    Some(format!(
        "env: {os_type} ({os_arch}) | login_status: {}",
        login_status_summary(login_status)
    ))
}

fn login_status_summary(v: &serde_json::Value) -> String {
    if v.get("LoggedOut").is_some() {
        return "logged_out".to_string();
    }
    if let Some(obj) = v.get("LoggedIn").and_then(|x| x.as_object()) {
        if let Some(user) = obj.get("user_id").and_then(|x| x.as_str()) {
            return format!("logged_in({})", &user[..user.len().min(8)]);
        }
        return "logged_in".to_string();
    }
    "unknown".to_string()
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
    let url = format!(
        "{}/api/task-attempts?task_id={task_id}",
        base_url.trim_end_matches('/')
    );

    let client = match reqwest::Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(format!("reqwest init: {e}")))
                .await;
            return;
        }
    };

    match client.get(url).send().await {
        Ok(resp) => match resp.json::<ApiResponse<Vec<WorkspaceDto>>>().await {
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
                    .send(NetEvent::Error(format!(
                        "failed to parse task attempts response: {e}"
                    )))
                    .await;
            }
        },
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(format!(
                    "failed to fetch task attempts: {e}"
                )))
                .await;
        }
    }
}
