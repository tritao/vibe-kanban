use crate::{
    events::{NetEvent, NetOpError},
    net::ops::update_executor_profile_http,
    state::{AppState, ExecutorProfileSelection},
};

pub(super) fn handle_executor_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    if tokens.len() == 1 {
        let current = app
            .ui
            .selected_executor_profile
            .as_ref()
            .map(|p| match p.variant.as_deref() {
                Some(v) if !v.trim().is_empty() => format!("{}:{v}", p.executor),
                _ => p.executor.clone(),
            })
            .unwrap_or_else(|| "unknown".to_string());

        if app.ui.available_executors.is_empty() {
            return Err(
                "no executor profiles loaded yet (try reconnect or wait for /api/info)".to_string(),
            );
        }
        let list = app.ui.available_executors.join(", ");
        app.ui
            .set_notice(format!("Executor: {current}\nAvailable: {list}"));
        return Ok(());
    }

    let exec = tokens[1].trim();
    if exec.is_empty() {
        return Err("usage: /executor <NAME> [--variant V]".to_string());
    }

    let help = crate::slash::help_syntax_for_command("executor")
        .unwrap_or("/executor <name> [--variant V]");
    let parsed =
        crate::slash::parse_flags(tokens, 2, crate::slash::flags_for_command("executor"), help)?;
    let variant = parsed.get_value("--variant").map(ToString::to_string);

    if !app.ui.available_executors.is_empty()
        && !app
            .ui
            .available_executors
            .iter()
            .any(|e| e.eq_ignore_ascii_case(exec))
    {
        return Err(format!("unknown executor: {exec} (try /executor)"));
    }

    let selection = ExecutorProfileSelection {
        executor: exec.to_string(),
        variant: variant.clone().filter(|s| !s.trim().is_empty()),
    };
    app.ui.selected_executor_profile = Some(selection.clone());
    app.ui.set_notice(format!(
        "Setting executor profile: {}{}",
        selection.executor,
        selection
            .variant
            .as_deref()
            .map(|v| format!(":{v}"))
            .unwrap_or_default()
    ));

    crate::commands::run_net_job(
        app,
        crate::state::JobKey::ExecutorProfile,
        move |base_url, net_tx| async move {
            match update_executor_profile_http(&base_url, &selection).await {
                Ok(()) => {
                    let _ = net_tx
                        .send(NetEvent::Notice("Executor profile updated.".to_string()))
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetOpError::new("update executor profile", e).into_event())
                        .await;
                }
            }
        },
    );

    Ok(())
}
