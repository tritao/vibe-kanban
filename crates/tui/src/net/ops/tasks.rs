use uuid::Uuid;

use crate::{
    net::api_client::{decode_api_response, http_client, url},
    state::TaskStatus,
};

pub(crate) async fn update_task_status_http(
    base_url: &str,
    task_id: Uuid,
    status: TaskStatus,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(base_url, &format!("/api/tasks/{task_id}"));
    let body = serde_json::json!({ "status": status.as_api_str() });

    let resp = client.put(endpoint).json(&body).send().await?;
    let api = decode_api_response::<()>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected status update");
    }
    Ok(())
}

pub(crate) async fn delete_task_http(
    base_url: &str,
    task_id: Uuid,
    delete_mode: Option<&str>,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let mut endpoint = url(base_url, &format!("/api/tasks/{task_id}"));
    if let Some(mode) = delete_mode.filter(|s| !s.trim().is_empty()) {
        endpoint.push_str(&format!("?delete_mode={mode}"));
    }

    let resp = client.delete(endpoint).send().await?;
    let api = decode_api_response::<()>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected delete task");
    }
    Ok(())
}

pub(crate) async fn create_task_http(
    base_url: &str,
    project_id: Uuid,
    title: &str,
    description: Option<&str>,
    status: TaskStatus,
    parent_task_id: Option<Uuid>,
) -> anyhow::Result<Uuid> {
    let client = http_client()?;

    let endpoint = url(base_url, "/api/tasks");
    let body = serde_json::json!({
        "project_id": project_id,
        "title": title,
        "description": description,
        "status": status.as_api_str(),
        "parent_task_id": parent_task_id,
        "parent_workspace_id": null,
        "image_ids": null,
        "shared_task_id": null,
    });

    #[derive(Debug, serde::Deserialize)]
    struct CreatedTaskDto {
        id: Uuid,
    }

    let resp = client.post(endpoint).json(&body).send().await?;
    let api = decode_api_response::<CreatedTaskDto>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected create task");
    }
    api.into_data()
        .map(|t| t.id)
        .ok_or_else(|| anyhow::anyhow!("missing task in create response"))
}
