use crate::{
    events::{NetEvent, NetOpError},
    net::ops::delete_task_http,
    state::AppState,
};

pub(super) fn handle_delete_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let Some(task_id) = app.board.selected_task_id else {
        return Err(crate::ui::messages::errors::NO_TASK_SELECTED.to_string());
    };

    let help = crate::slash::help_syntax_for_command("delete").unwrap_or("/delete [--subtree]");
    let parsed =
        crate::slash::parse_flags(tokens, 1, crate::slash::flags_for_command("delete"), help)?;
    let mode = if parsed.get_bool("--subtree") {
        Some("subtree")
    } else {
        Some("promote")
    };

    crate::commands::run_net_job(
        app,
        crate::state::JobKey::TaskDelete,
        move |base_url, net_tx| async move {
            match delete_task_http(&base_url, task_id, mode).await {
                Ok(()) => {
                    let _ = net_tx
                        .send(NetEvent::Notice("Deleted task.".to_string()))
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetOpError::new("delete task", e).into_event())
                        .await;
                }
            }
        },
    );

    Ok(())
}
