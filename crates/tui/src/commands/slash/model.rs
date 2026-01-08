use crate::{
    events::NetEvent, net::ops::update_model_settings_http, state::AppState,
    store::executor_profiles::ExecutorProfilesStore,
};

pub(super) fn handle_model_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let Some(selection) = app.ui.selected_executor_profile.clone() else {
        return Err("no executor selected (try /executor)".to_string());
    };

    if tokens.len() == 1 {
        let store = ExecutorProfilesStore::new(&app.ui.executor_profiles);
        let (model, effort) = store.current_model_and_effort(&selection);
        let model = model.unwrap_or_else(|| "unset".to_string());
        let effort = effort.unwrap_or_else(|| "unset".to_string());
        app.ui.set_notice(format!(
            "Model: {model}\nReasoning effort: {effort}\n\nSet: /model <MODEL> [--effort E]"
        ));
        return Ok(());
    }

    let mut model: Option<String> = None;
    let mut flags_start = 1;
    if let Some(tok) = tokens.get(1).map(|s| s.as_str()) {
        if !tok.starts_with("--") {
            model = Some(tok.to_string());
            flags_start = 2;
        }
    }

    let help =
        crate::slash::help_syntax_for_command("model").unwrap_or("/model <MODEL> [--effort E]");
    let parsed = crate::slash::parse_flags(
        tokens,
        flags_start,
        crate::slash::flags_for_command("model"),
        help,
    )?;
    let effort = parsed.get_value("--effort").map(ToString::to_string);

    let model_empty = model
        .as_deref()
        .map(|m| m.trim().is_empty())
        .unwrap_or(true);
    let effort_empty = effort
        .as_deref()
        .map(|e| e.trim().is_empty())
        .unwrap_or(true);
    if model_empty && effort_empty {
        return Err("usage: /model <MODEL> [--effort E]".to_string());
    }

    let desc = match (model.as_deref(), effort.as_deref()) {
        (Some(m), Some(e)) => format!("{m} (effort {e})"),
        (Some(m), None) => m.to_string(),
        (None, Some(e)) => format!("(effort {e})"),
        (None, None) => "unknown".to_string(),
    };
    app.ui.set_notice(format!(
        "Updating model settings for {}{}: {desc}",
        selection.executor,
        selection
            .variant
            .as_deref()
            .map(|v| format!(":{v}"))
            .unwrap_or_default()
    ));

    let selection2 = selection.clone();
    crate::commands::run_net_job(
        app,
        crate::state::JobKey::ModelSettings,
        move |base_url, net_tx| async move {
            match update_model_settings_http(
                &base_url,
                &selection2,
                model.as_deref(),
                effort.as_deref(),
            )
            .await
            {
                Ok(()) => {
                    let info_tx = net_tx.clone();
                    let info_url = base_url.clone();
                    let _ = net_tx
                        .send(NetEvent::Notice("Model settings updated.".to_string()))
                        .await;
                    tokio::spawn(crate::net::load_info_task(info_url, info_tx));
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!(
                            "failed to update model settings: {e}"
                        )))
                        .await;
                }
            }
        },
    );

    Ok(())
}
