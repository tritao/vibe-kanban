use crate::{events::NetEvent, net::ops::update_model_settings_http, state::AppState};

fn current_model_and_effort(
    profiles_executors: &serde_json::Value,
    selection: &crate::state::ExecutorProfileSelection,
) -> (Option<String>, Option<String>) {
    let Some(execs) = profiles_executors.as_object() else {
        return (None, None);
    };
    let exec_key = execs
        .keys()
        .find(|k| k.eq_ignore_ascii_case(&selection.executor))
        .cloned()
        .unwrap_or_else(|| selection.executor.clone());
    let Some(variants) = execs.get(&exec_key).and_then(|v| v.as_object()) else {
        return (None, None);
    };
    let wanted_variant = selection.variant.as_deref().unwrap_or("DEFAULT");
    let variant_key = variants
        .keys()
        .find(|k| k.eq_ignore_ascii_case(wanted_variant))
        .cloned()
        .unwrap_or_else(|| wanted_variant.to_string());
    let Some(variant) = variants.get(&variant_key).and_then(|v| v.as_object()) else {
        return (None, None);
    };
    let nested_key = variant
        .keys()
        .find(|k| k.eq_ignore_ascii_case(&exec_key))
        .cloned()
        .unwrap_or_else(|| exec_key.clone());
    let Some(cfg) = variant.get(&nested_key).and_then(|v| v.as_object()) else {
        return (None, None);
    };

    let model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let effort = cfg
        .get("model_reasoning_effort")
        .or_else(|| cfg.get("reasoning_effort"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    (model, effort)
}

pub(super) fn handle_model_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let Some(selection) = app.ui.selected_executor_profile.clone() else {
        return Err("no executor selected (try /executor)".to_string());
    };

    if tokens.len() == 1 {
        let (model, effort) = current_model_and_effort(&app.ui.executor_profiles, &selection);
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
    crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
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
    });

    Ok(())
}
