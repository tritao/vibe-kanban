use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite;
use uuid::Uuid;

use super::common::{WsConnectError, WsParsed, connect_ws_detailed, parse_ws_message};
use crate::{
    events::{NetEvent, StreamStatus},
    state::{LogMode, UiMessageKey},
};

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
            "{}/api/executions/{exec_id}/logs/ws?mode={}",
            base_url.trim_end_matches('/'),
            log_mode.label()
        );

        let _ = net_tx.send(NetEvent::LogReset(Some(exec_id))).await;
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
                    .send(NetEvent::ErrorKey {
                        key: UiMessageKey::LogStreamConnect,
                        message: format!("log stream connect: {e}"),
                    })
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
