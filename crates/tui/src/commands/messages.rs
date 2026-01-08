use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    events::NetEvent,
    net::ops::{follow_up_http, queue_follow_up_http},
    state::ExecutorProfileSelection,
};

#[derive(Clone)]
pub(crate) struct SendUserMessage {
    pub(crate) session_id: Option<Uuid>,
    pub(crate) attempt_id: Option<Uuid>,
    pub(crate) task_id: Option<Uuid>,
    pub(crate) project_id: Option<Uuid>,
    pub(crate) executor_profile: Option<ExecutorProfileSelection>,
    pub(crate) text: String,
    pub(crate) queue_if_running: bool,
}

pub(crate) async fn send_user_message_task(
    base_url: String,
    net_tx: mpsc::Sender<NetEvent>,
    mut msg: SendUserMessage,
) {
    let session_id = match msg.session_id {
        Some(id) => Some(id),
        None => {
            crate::commands::ensure_session_id_for_message(
                &base_url,
                &net_tx,
                msg.attempt_id,
                msg.task_id,
                msg.project_id,
                msg.executor_profile.take(),
            )
            .await
        }
    };

    let Some(session_id) = session_id else {
        return;
    };

    let result = if msg.queue_if_running {
        queue_follow_up_http(&base_url, session_id, &msg.text).await
    } else {
        follow_up_http(&base_url, session_id, &msg.text).await
    };

    if let Err(e) = result {
        let _ = net_tx
            .send(NetEvent::Error(crate::fmt::op_failed("follow-up", e)))
            .await;
    }
}
