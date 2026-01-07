use anyhow::Context;
use uuid::Uuid;

use crate::net::{
    api_client::{decode_api_response, http_client, url},
    ops::wire::ApiResponseWire,
};

pub(crate) async fn stop_exec_http(base_url: &str, exec_id: Uuid) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/execution-processes/{exec_id}/stop"),
    );
    let resp = client.post(endpoint).send().await?;
    let api = decode_api_response::<serde_json::Value>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected stop request");
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SessionDto {
    pub(crate) id: Uuid,
}

pub(crate) async fn latest_session_id_http(
    base_url: &str,
    workspace_id: Uuid,
) -> anyhow::Result<Option<Uuid>> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/sessions?workspace_id={workspace_id}"),
    );

    let resp = client.get(endpoint).send().await?;
    let api = decode_api_response::<Vec<SessionDto>>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected sessions request");
    }
    Ok(api
        .into_data()
        .unwrap_or_default()
        .into_iter()
        .next()
        .map(|s| s.id))
}

pub(crate) async fn queue_follow_up_http(
    base_url: &str,
    session_id: Uuid,
    message: &str,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(base_url, &format!("/api/sessions/{session_id}/queue"));
    let body = serde_json::json!({ "message": message, "variant": null });

    let resp = client.post(endpoint).json(&body).send().await?;
    let status = resp.status();
    let body_text = resp.text().await.unwrap_or_default();
    let api: ApiResponseWire<serde_json::Value> = serde_json::from_str(&body_text)
        .with_context(|| format!("parse backend response (status {status})"))?;
    if !api.success {
        let msg = api
            .message
            .unwrap_or_else(|| "backend rejected queue request".to_string());
        anyhow::bail!("{msg} (HTTP {status})");
    }
    Ok(())
}

pub(crate) async fn follow_up_http(
    base_url: &str,
    session_id: Uuid,
    prompt: &str,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(base_url, &format!("/api/sessions/{session_id}/follow-up"));
    let body = serde_json::json!({
        "prompt": prompt,
        "variant": null,
        "retry_process_id": null,
        "force_when_dirty": null,
        "perform_git_reset": null,
    });

    let resp = client.post(endpoint).json(&body).send().await?;
    let status = resp.status();
    let body_text = resp.text().await.unwrap_or_default();
    let api: ApiResponseWire<serde_json::Value> = serde_json::from_str(&body_text)
        .with_context(|| format!("parse backend response (status {status})"))?;
    if !api.success {
        let msg = api
            .message
            .unwrap_or_else(|| "backend rejected follow-up request".to_string());
        anyhow::bail!("{msg} (HTTP {status})");
    }
    Ok(())
}
