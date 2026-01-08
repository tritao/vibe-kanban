use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite;
use uuid::Uuid;

use super::common::{WsParsed, connect_ws, parse_ws_message};
use crate::events::{NetEvent, NetOpError, StreamStatus};

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
                                        let _ = net_tx
                                            .send(
                                                NetOpError::new(
                                                    "exec stream message",
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
                                                "exec stream",
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
                    .send(NetEvent::ExecStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::ExecStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetOpError::new("exec stream connect", e).into_event())
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
