use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite;
use uuid::Uuid;

use super::common::{WsParsed, connect_ws, parse_ws_message};
use crate::events::{NetEvent, StreamStatus};

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
