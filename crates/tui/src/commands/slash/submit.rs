use crate::{
    events::NetEvent,
    logs::{append_local_user_message, set_pending_user_log},
    net::ops::{follow_up_http, queue_follow_up_http},
    selection::exec_list,
    state::AppState,
};

#[derive(Debug)]
enum ComposerSubmit {
    Quit,
    Slash(Vec<String>),
    Message,
}

fn parse_composer_submit(message: &str) -> Result<ComposerSubmit, String> {
    if crate::slash::composer_is_slash_mode(message) {
        let cmdline = message.trim_start().trim_start_matches('/');
        let tokens = crate::cli_parse::tokenize_command_line(cmdline)
            .map_err(|e| format!("invalid command: {e}"))?;
        if tokens.is_empty() {
            return Err("invalid command: empty".to_string());
        }
        let first = tokens[0].as_str();
        let cmd = crate::slash::canonical_command_name(first).unwrap_or(first);
        if cmd == "quit" {
            return Ok(ComposerSubmit::Quit);
        }
        return Ok(ComposerSubmit::Slash(tokens));
    }
    Ok(ComposerSubmit::Message)
}

pub(crate) fn submit_composer(app: &mut AppState) -> bool {
    let msg = app.ui.composer.buffer.trim_end().to_string();
    if msg.trim().is_empty() {
        app.ui.composer_active = false;
        app.ui.composer.clear();
        return false;
    }

    let refresh_branch_status_after_send = app.ui.refresh_branch_status_after_send;
    app.ui.refresh_branch_status_after_send = false;

    let parsed = match parse_composer_submit(&msg) {
        Ok(p) => p,
        Err(e) => {
            app.ui.set_error(e);
            return false;
        }
    };

    // Keep a local record of what the user sent in the run logs, since the backend log stream
    // does not always include user messages.
    let mut execs_for_log = exec_list(&app.exec.exec_store);
    execs_for_log.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let current_exec_id = app
        .exec
        .selected_exec_id
        .or_else(|| execs_for_log.last().map(|e| e.id));

    if matches!(parsed, ComposerSubmit::Quit) {
        app.ui.composer_active = false;
        app.ui.composer.clear();
        return true;
    }

    if matches!(parsed, ComposerSubmit::Slash(_)) {
        if let Some(exec_id) = current_exec_id {
            append_local_user_message(app, exec_id, &msg);
        }
        app.ui.composer_active = false;
        app.ui.composer.clear();
        let ComposerSubmit::Slash(tokens) = parsed else {
            unreachable!();
        };
        return submit_slash_tokens(app, &tokens);
    }

    let attempt_id = app.board.selected_attempt_id;
    let task_id = app.board.selected_task_id;
    let project_id = app.board.selected_project_id;
    let executor_profile = app.ui.selected_executor_profile.clone();

    let execs = exec_list(&app.exec.exec_store);
    let active = app
        .exec
        .selected_exec_id
        .and_then(|id| execs.iter().find(|e| e.id == id));
    let session_id = active.and_then(|e| e.session_id);
    let is_running = active.is_some_and(|e| e.status == Some(crate::state::ExecStatus::Running));

    if session_id.is_none() && attempt_id.is_none() && task_id.is_none() {
        app.ui.set_error(
            "No task/attempt selected. Create/select a task first (press `n` to create a task)."
                .to_string(),
        );
        return false;
    }

    app.ui.composer_active = false;
    app.ui.composer.clear();

    if let Some(exec_id) = current_exec_id {
        if is_running {
            append_local_user_message(app, exec_id, &msg);
        } else {
            set_pending_user_log(app, msg.clone());
        }
    }
    if refresh_branch_status_after_send {
        if is_running {
            if let Some(exec_id) = current_exec_id {
                crate::commands::arm_branch_status_refresh_for_exec(app, exec_id);
            }
        } else {
            crate::commands::arm_branch_status_refresh_after_next_exec(app, current_exec_id);
        }
    }

    crate::commands::run_net_job_one_shot(app, move |base_url, net_tx| async move {
        let session_id = match session_id {
            Some(id) => Some(id),
            None => {
                crate::commands::ensure_session_id_for_message(
                    &base_url,
                    &net_tx,
                    attempt_id,
                    task_id,
                    project_id,
                    executor_profile,
                )
                .await
            }
        };

        let Some(session_id) = session_id else {
            return;
        };

        let result = if is_running {
            queue_follow_up_http(&base_url, session_id, &msg).await
        } else {
            follow_up_http(&base_url, session_id, &msg).await
        };

        if let Err(e) = result {
            let _ = net_tx
                .send(NetEvent::Error(crate::fmt::op_failed("follow-up", e)))
                .await;
        }
    });

    false
}

fn submit_slash_tokens(app: &mut AppState, tokens: &[String]) -> bool {
    match super::parse_slash_command(app, &tokens) {
        Ok(quit) => return quit,
        Err(e) => {
            app.ui.set_error(e);
        }
    }
    false
}
