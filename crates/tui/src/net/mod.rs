use std::time::Duration;

use anyhow::Context;
use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite;
use utils::{port_file::read_port_file, response::ApiResponse};
use uuid::Uuid;

use crate::{
    Args,
    events::{NetEvent, StreamStatus},
    state::{AttemptRow, ExecutorProfileSelection, LogMode},
};

pub(crate) mod api_client;
pub(crate) mod ops;

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

pub(crate) async fn projects_stream_task(
    base_url: String,
    mut reconnect_rx: watch::Receiver<u64>,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let mut backoff = Duration::from_millis(250);
    let max_backoff = Duration::from_secs(8);
    let endpoint = format!("{}/api/projects/stream/ws", base_url.trim_end_matches('/'));

    loop {
        let _ = net_tx
            .send(NetEvent::ProjectsStreamStatus(StreamStatus::Connecting))
            .await;

        match connect_ws(&endpoint).await {
            Ok(mut stream) => {
                let _ = net_tx
                    .send(NetEvent::ProjectsStreamStatus(StreamStatus::Connected))
                    .await;
                backoff = Duration::from_millis(250);

                loop {
                    tokio::select! {
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            break;
                        }
                        msg = stream.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(tungstenite::Message::Text(text)) => match parse_ws_message(&text) {
                                    WsParsed::Patch(patch) => {
                                        let _ = net_tx.send(NetEvent::ProjectsPatch(patch)).await;
                                    }
                                    WsParsed::Finished => {
                                        let _ = net_tx
                                            .send(NetEvent::ProjectsStreamStatus(
                                                StreamStatus::Disconnected,
                                            ))
                                            .await;
                                        return;
                                    }
                                    WsParsed::Ignored => {}
                                    WsParsed::Error(e) => {
                                        let _ = net_tx
                                            .send(NetEvent::Error(format!(
                                                "projects stream message error: {e}"
                                            )))
                                            .await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx
                                        .send(NetEvent::Error(format!("projects stream: {e}")))
                                        .await;
                                    break;
                                }
                            }
                        }
                    }
                }

                let _ = net_tx
                    .send(NetEvent::ProjectsStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::ProjectsStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetEvent::Error(format!("projects stream connect: {e}")))
                    .await;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            changed = reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
            }
        }
        backoff = (backoff * 2).min(max_backoff);
    }
}

pub(crate) async fn tasks_stream_task(
    base_url: String,
    mut project_rx: watch::Receiver<Option<Uuid>>,
    mut reconnect_rx: watch::Receiver<u64>,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let max_backoff = Duration::from_secs(8);
    let mut backoff = Duration::from_millis(250);

    loop {
        let project_id = *project_rx.borrow();
        let Some(project_id) = project_id else {
            let _ = net_tx.send(NetEvent::TasksReset).await;
            let _ = net_tx
                .send(NetEvent::TasksStreamStatus(StreamStatus::Disconnected))
                .await;
            tokio::select! {
                changed = project_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = reconnect_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
            continue;
        };

        let endpoint = format!(
            "{}/api/tasks/stream/ws?project_id={project_id}",
            base_url.trim_end_matches('/')
        );

        let _ = net_tx
            .send(NetEvent::TasksStreamStatus(StreamStatus::Connecting))
            .await;
        let _ = net_tx.send(NetEvent::TasksReset).await;

        match connect_ws(&endpoint).await {
            Ok(mut stream) => {
                let _ = net_tx
                    .send(NetEvent::TasksStreamStatus(StreamStatus::Connected))
                    .await;
                backoff = Duration::from_millis(250);

                loop {
                    tokio::select! {
                        changed = project_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::TasksReset).await;
                            break;
                        }
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::TasksReset).await;
                            break;
                        }
                        msg = stream.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(tungstenite::Message::Text(text)) => match parse_ws_message(&text) {
                                    WsParsed::Patch(patch) => {
                                        let _ = net_tx.send(NetEvent::TasksPatch(patch)).await;
                                    }
                                    WsParsed::Finished => {
                                        let _ = net_tx.send(NetEvent::TasksStreamStatus(StreamStatus::Disconnected)).await;
                                        return;
                                    }
                                    WsParsed::Ignored => {}
                                    WsParsed::Error(e) => {
                                        let _ = net_tx.send(NetEvent::Error(format!("tasks stream message error: {e}"))).await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx.send(NetEvent::Error(format!("tasks stream: {e}"))).await;
                                    break;
                                }
                            }
                        }
                    }
                }

                let _ = net_tx
                    .send(NetEvent::TasksStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::TasksStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetEvent::Error(format!("tasks stream connect: {e}")))
                    .await;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            changed = project_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
        }
        backoff = (backoff * 2).min(max_backoff);
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

pub(crate) async fn exec_stream_task(
    base_url: String,
    mut attempt_rx: watch::Receiver<Option<Uuid>>,
    mut reconnect_rx: watch::Receiver<u64>,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let max_backoff = Duration::from_secs(8);
    let mut backoff = Duration::from_millis(250);

    loop {
        let attempt_id = *attempt_rx.borrow();
        let Some(attempt_id) = attempt_id else {
            let _ = net_tx.send(NetEvent::ExecReset).await;
            let _ = net_tx
                .send(NetEvent::ExecStreamStatus(StreamStatus::Disconnected))
                .await;
            tokio::select! {
                changed = attempt_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = reconnect_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
            continue;
        };

        let endpoint = format!(
            "{}/api/execution-processes/stream/ws?workspace_id={attempt_id}",
            base_url.trim_end_matches('/')
        );

        let _ = net_tx
            .send(NetEvent::ExecStreamStatus(StreamStatus::Connecting))
            .await;
        let _ = net_tx.send(NetEvent::ExecReset).await;

        match connect_ws(&endpoint).await {
            Ok(mut stream) => {
                let _ = net_tx
                    .send(NetEvent::ExecStreamStatus(StreamStatus::Connected))
                    .await;
                backoff = Duration::from_millis(250);

                loop {
                    tokio::select! {
                        changed = attempt_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::ExecReset).await;
                            break;
                        }
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::ExecReset).await;
                            break;
                        }
                        msg = stream.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(tungstenite::Message::Text(text)) => match parse_ws_message(&text) {
                                    WsParsed::Patch(patch) => {
                                        let _ = net_tx.send(NetEvent::ExecPatch(patch)).await;
                                    }
                                    WsParsed::Finished => {
                                        let _ = net_tx.send(NetEvent::ExecStreamStatus(StreamStatus::Disconnected)).await;
                                        return;
                                    }
                                    WsParsed::Ignored => {}
                                    WsParsed::Error(e) => {
                                        let _ = net_tx.send(NetEvent::Error(format!("exec stream message error: {e}"))).await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx.send(NetEvent::Error(format!("exec stream: {e}"))).await;
                                    break;
                                }
                            }
                        }
                    }
                }

                let _ = net_tx
                    .send(NetEvent::ExecStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::ExecStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetEvent::Error(format!("exec stream connect: {e}")))
                    .await;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            changed = attempt_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
        }
        backoff = (backoff * 2).min(max_backoff);
    }
}

pub(crate) async fn diff_stream_task(
    base_url: String,
    mut attempt_rx: watch::Receiver<Option<Uuid>>,
    mut stats_only_rx: watch::Receiver<bool>,
    mut diff_reconnect_rx: watch::Receiver<u64>,
    mut reconnect_rx: watch::Receiver<u64>,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let max_backoff = Duration::from_secs(8);
    let mut backoff = Duration::from_millis(250);

    loop {
        let attempt_id = *attempt_rx.borrow();
        let Some(attempt_id) = attempt_id else {
            let _ = net_tx.send(NetEvent::DiffReset).await;
            let _ = net_tx
                .send(NetEvent::DiffStreamStatus(StreamStatus::Disconnected))
                .await;
            tokio::select! {
                changed = attempt_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = stats_only_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = diff_reconnect_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = reconnect_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
            continue;
        };

        let stats_only = *stats_only_rx.borrow();
        let endpoint = format!(
            "{}/api/task-attempts/{attempt_id}/diff/ws?stats_only={stats_only}",
            base_url.trim_end_matches('/')
        );

        let _ = net_tx
            .send(NetEvent::DiffStreamStatus(StreamStatus::Connecting))
            .await;
        let _ = net_tx.send(NetEvent::DiffReset).await;

        match connect_ws(&endpoint).await {
            Ok(mut stream) => {
                let _ = net_tx
                    .send(NetEvent::DiffStreamStatus(StreamStatus::Connected))
                    .await;
                backoff = Duration::from_millis(250);

                loop {
                    tokio::select! {
                        changed = attempt_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::DiffReset).await;
                            break;
                        }
                        changed = stats_only_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::DiffReset).await;
                            break;
                        }
                        changed = diff_reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::DiffReset).await;
                            break;
                        }
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::DiffReset).await;
                            break;
                        }
                        msg = stream.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(tungstenite::Message::Text(text)) => match parse_ws_message(&text) {
                                    WsParsed::Patch(patch) => {
                                        let _ = net_tx.send(NetEvent::DiffPatch(patch)).await;
                                    }
                                    WsParsed::Finished | WsParsed::Ignored => {}
                                    WsParsed::Error(e) => {
                                        let _ = net_tx.send(NetEvent::Error(format!("diff stream parse: {e}"))).await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx.send(NetEvent::Error(format!("diff stream: {e}"))).await;
                                    break;
                                }
                            }
                        }
                    }
                }

                let _ = net_tx
                    .send(NetEvent::DiffStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::DiffStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetEvent::Error(format!("diff stream connect: {e}")))
                    .await;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            changed = attempt_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = stats_only_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = diff_reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
        }
        backoff = (backoff * 2).min(max_backoff);
    }
}

pub(crate) async fn logs_stream_task(
    base_url: String,
    mut exec_rx: watch::Receiver<Option<Uuid>>,
    mut log_mode_rx: watch::Receiver<LogMode>,
    mut reconnect_rx: watch::Receiver<u64>,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let max_backoff = Duration::from_secs(8);
    let mut backoff = Duration::from_millis(250);

    loop {
        let exec_id = *exec_rx.borrow();
        let Some(exec_id) = exec_id else {
            let _ = net_tx.send(NetEvent::LogReset(None)).await;
            let _ = net_tx
                .send(NetEvent::LogStreamStatus(StreamStatus::Disconnected))
                .await;
            tokio::select! {
                changed = exec_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = log_mode_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = reconnect_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
            continue;
        };

        let log_mode = *log_mode_rx.borrow();
        let endpoint = format!(
            "{}/api/execution-processes/{exec_id}/{}-logs/ws",
            base_url.trim_end_matches('/'),
            match log_mode {
                LogMode::Normalized => "normalized",
                LogMode::Raw => "raw",
            }
        );

        let _ = net_tx
            .send(NetEvent::LogStreamStatus(StreamStatus::Connecting))
            .await;

        match connect_ws_detailed(&endpoint).await {
            Ok(mut stream) => {
                let _ = net_tx
                    .send(NetEvent::LogStreamStatus(StreamStatus::Connected))
                    .await;
                backoff = Duration::from_millis(250);

                let mut finished = false;
                loop {
                    tokio::select! {
                        changed = exec_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            break;
                        }
                        changed = log_mode_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::LogReset(None)).await;
                            break;
                        }
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            break;
                        }
                        msg = stream.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(tungstenite::Message::Text(text)) => match parse_ws_message(&text) {
                                    WsParsed::Patch(patch) => {
                                        let _ = net_tx.send(NetEvent::LogPatch { exec_id, patch }).await;
                                    }
                                    WsParsed::Finished => {
                                        finished = true;
                                        break;
                                    }
                                    WsParsed::Ignored => {}
                                    WsParsed::Error(e) => {
                                        let _ = net_tx.send(NetEvent::Error(format!("log stream message error: {e}"))).await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx.send(NetEvent::Error(format!("log stream: {e}"))).await;
                                    break;
                                }
                            }
                        }
                    }
                }

                if finished {
                    let _ = net_tx
                        .send(NetEvent::LogStreamStatus(StreamStatus::Completed))
                        .await;
                    tokio::select! {
                        changed = exec_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                        }
                        changed = log_mode_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                        }
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                        }
                    }
                    continue;
                }

                let _ = net_tx
                    .send(NetEvent::LogStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(WsConnectError::HttpStatus(404)) => {
                // This can happen briefly right after a new execution is created, before the
                // log stream endpoint is available. Treat as transient and retry quickly.
                let _ = net_tx
                    .send(NetEvent::LogStreamStatus(StreamStatus::Disconnected))
                    .await;
                backoff = Duration::from_millis(250);
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::LogStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetEvent::Error(format!("log stream connect: {e}")))
                    .await;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            changed = exec_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = log_mode_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
        }
        backoff = (backoff * 2).min(max_backoff);
    }
}

#[derive(Debug)]
enum WsConnectError {
    HttpStatus(u16),
    Other(anyhow::Error),
}

impl std::fmt::Display for WsConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HttpStatus(code) => write!(f, "HTTP {code}"),
            Self::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for WsConnectError {}

async fn connect_ws_detailed(
    http_url: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    WsConnectError,
> {
    let ws_url = if let Some(rest) = http_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if let Some(rest) = http_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if http_url.starts_with("ws://") || http_url.starts_with("wss://") {
        http_url.to_string()
    } else {
        return Err(WsConnectError::Other(anyhow::anyhow!(
            "unsupported URL scheme: {http_url}"
        )));
    };

    match tokio_tungstenite::connect_async(ws_url).await {
        Ok((ws, _resp)) => Ok(ws),
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            Err(WsConnectError::HttpStatus(resp.status().as_u16()))
        }
        Err(e) => Err(WsConnectError::Other(anyhow::Error::new(e))),
    }
}

async fn connect_ws(
    http_url: &str,
) -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let ws_url = if let Some(rest) = http_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if let Some(rest) = http_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if http_url.starts_with("ws://") || http_url.starts_with("wss://") {
        http_url.to_string()
    } else {
        anyhow::bail!("unsupported URL scheme: {http_url}");
    };

    let (ws, _resp) = tokio_tungstenite::connect_async(ws_url).await?;
    Ok(ws)
}

enum WsParsed {
    Patch(json_patch::Patch),
    Finished,
    Ignored,
    Error(String),
}

fn parse_ws_message(text: &str) -> WsParsed {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return WsParsed::Error(e.to_string()),
    };

    if value.get("finished").and_then(|v| v.as_bool()) == Some(true) {
        return WsParsed::Finished;
    }

    let patch_value = match value.get("JsonPatch") {
        Some(v) => v.clone(),
        None => return WsParsed::Ignored,
    };

    let patch: json_patch::Patch = match serde_json::from_value(patch_value) {
        Ok(p) => p,
        Err(e) => return WsParsed::Error(e.to_string()),
    };

    WsParsed::Patch(patch)
}
