use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite;
use uuid::Uuid;

use super::common::{WsParsed, connect_ws, parse_ws_message};
use crate::events::{NetEvent, NetOpError, StreamStatus};

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
                                        let _ = net_tx
                                            .send(
                                                NetOpError::new(
                                                    "diff stream message",
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
                                                "diff stream",
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
                    .send(NetEvent::DiffStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::DiffStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetOpError::new("diff stream connect", e).into_event())
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
