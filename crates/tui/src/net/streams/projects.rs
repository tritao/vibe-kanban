use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite;

use super::common::{WsParsed, connect_ws, parse_ws_message};
use crate::events::{NetEvent, NetOpError, StreamStatus};

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
                                            .send(
                                                NetOpError::new(
                                                    "projects stream message",
                                                    anyhow::anyhow!(e),
                                                )
                                                .into_event(),
                                            )
                                            .await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx
                                        .send(
                                            NetOpError::new(
                                                "projects stream",
                                                anyhow::Error::new(e),
                                            )
                                            .into_event(),
                                        )
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
                    .send(NetOpError::new("projects stream connect", e).into_event())
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
