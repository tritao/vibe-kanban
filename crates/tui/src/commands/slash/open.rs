use super::require_selected_attempt_id;
use crate::{
    events::{NetEvent, NetOpError},
    net::ops::open_editor_http,
    state::AppState,
};

pub(super) fn handle_open_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    if tokens.len() < 2 {
        return Err(crate::slash::usage_for_command("open")
            .unwrap_or("usage: /open <file_path>")
            .to_string());
    }
    let attempt_id = require_selected_attempt_id(app)?;
    let file_path = tokens[1].clone();
    crate::commands::run_net_job(
        app,
        crate::state::JobKey::OpenEditor,
        move |base_url, net_tx| async move {
            match open_editor_http(&base_url, attempt_id, Some(file_path.clone())).await {
                Ok(url) => {
                    let msg = match url {
                        Some(url) => format!("Opened editor for {file_path}: {url}"),
                        None => format!("Opened editor for {file_path}."),
                    };
                    let _ = net_tx.send(NetEvent::Notice(msg)).await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetOpError::new("open editor", e).into_event())
                        .await;
                }
            }
        },
    );
    Ok(())
}
