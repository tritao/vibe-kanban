use anyhow::Context;

use crate::{
    net::api_client::{decode_api_response, http_client, url},
    state::ExecutorProfileSelection,
};

pub(crate) async fn update_executor_profile_http(
    base_url: &str,
    profile: &ExecutorProfileSelection,
) -> anyhow::Result<()> {
    let client = http_client()?;

    // Fetch current config from /api/info so we can PUT the full config object.
    let info_url = url(base_url, "/api/info");
    let resp = client.get(info_url).send().await?;
    let api = decode_api_response::<serde_json::Value>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected info request");
    }
    let info = api
        .into_data()
        .ok_or_else(|| anyhow::anyhow!("missing info payload"))?;
    let mut config = info
        .get("config")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("missing config in info payload"))?;

    if let Some(obj) = config.as_object_mut() {
        obj.insert(
            "executor_profile".to_string(),
            serde_json::to_value(profile).context("serialize executor_profile")?,
        );
    } else {
        anyhow::bail!("invalid config payload");
    }

    let endpoint = url(base_url, "/api/config");
    let resp = client.put(endpoint).json(&config).send().await?;
    let api = decode_api_response::<serde_json::Value>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected config update");
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct ProfilesContentDto {
    content: String,
    #[allow(dead_code)]
    path: String,
}

fn find_key_case_insensitive(
    obj: &serde_json::Map<String, serde_json::Value>,
    needle: &str,
) -> Option<String> {
    obj.keys().find(|k| k.eq_ignore_ascii_case(needle)).cloned()
}

fn normalize_effort_for_executor(executor: &str, effort: &str) -> anyhow::Result<String> {
    let raw = effort.trim();
    if raw.is_empty() {
        anyhow::bail!("invalid effort: empty");
    }

    let normalized = raw.to_ascii_lowercase().replace('_', "-");
    let exec = executor.to_ascii_lowercase();

    let allowed: &[&str] = if exec == "codex" {
        &["low", "medium", "high", "xhigh"]
    } else if exec == "droid" {
        &["none", "dynamic", "off", "low", "medium", "high"]
    } else {
        &["low", "medium", "high"]
    };

    if allowed.iter().any(|a| *a == normalized) {
        Ok(normalized)
    } else {
        anyhow::bail!(
            "invalid reasoning effort: {raw} (allowed: {})",
            allowed.join(", ")
        );
    }
}

fn apply_model_settings_update(
    profiles: &mut serde_json::Value,
    selection: &ExecutorProfileSelection,
    model: Option<&str>,
    effort: Option<&str>,
) -> anyhow::Result<()> {
    let executors = profiles
        .get_mut("executors")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| anyhow::anyhow!("missing executors section in profiles"))?;

    let exec_key = find_key_case_insensitive(executors, &selection.executor)
        .ok_or_else(|| anyhow::anyhow!("unknown executor: {}", selection.executor))?;
    let variants = executors
        .get_mut(&exec_key)
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| anyhow::anyhow!("invalid profiles: executors.{exec_key}"))?;

    let wanted_variant = selection.variant.as_deref().unwrap_or("DEFAULT");
    let variant_key = find_key_case_insensitive(variants, wanted_variant)
        .ok_or_else(|| anyhow::anyhow!("unknown variant: {wanted_variant}"))?;
    let variant_obj = variants
        .get_mut(&variant_key)
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| anyhow::anyhow!("invalid profiles: executors.{exec_key}.{variant_key}"))?;

    let nested_key = find_key_case_insensitive(variant_obj, &exec_key)
        .ok_or_else(|| anyhow::anyhow!("invalid profiles: missing nested executor config"))?;
    let exec_cfg = variant_obj
        .get_mut(&nested_key)
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| anyhow::anyhow!("invalid profiles: nested executor config"))?;

    if let Some(model) = model.map(str::trim).filter(|s| !s.is_empty()) {
        exec_cfg.insert(
            "model".to_string(),
            serde_json::Value::String(model.to_string()),
        );
    }

    if let Some(effort) = effort.map(str::trim).filter(|s| !s.is_empty()) {
        let normalized = normalize_effort_for_executor(&exec_key, effort)?;

        let effort_key = if exec_cfg.contains_key("model_reasoning_effort") {
            "model_reasoning_effort"
        } else if exec_cfg.contains_key("reasoning_effort") {
            "reasoning_effort"
        } else if exec_key.eq_ignore_ascii_case("codex") {
            "model_reasoning_effort"
        } else {
            "reasoning_effort"
        };

        exec_cfg.insert(
            effort_key.to_string(),
            serde_json::Value::String(normalized),
        );
    }

    Ok(())
}

pub(crate) async fn update_model_settings_http(
    base_url: &str,
    selection: &ExecutorProfileSelection,
    model: Option<&str>,
    effort: Option<&str>,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(base_url, "/api/profiles");
    let resp = client.get(endpoint.clone()).send().await?;
    let api = decode_api_response::<ProfilesContentDto>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected profiles request");
    }
    let dto = api
        .into_data()
        .ok_or_else(|| anyhow::anyhow!("missing profiles payload"))?;

    let mut profiles: serde_json::Value =
        serde_json::from_str(&dto.content).context("parse profiles JSON")?;
    apply_model_settings_update(&mut profiles, selection, model, effort)?;
    let body = serde_json::to_string_pretty(&profiles).context("serialize profiles JSON")?;

    let resp = client
        .put(endpoint)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await?;
    let api = decode_api_response::<serde_json::Value>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected profiles update");
    }
    Ok(())
}
