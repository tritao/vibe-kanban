use uuid::Uuid;

use crate::net::{
    api_client::{decode_json_response, http_client, url},
    ops::wire::ApiResponseWire,
};

#[derive(Debug, serde::Serialize)]
pub(crate) struct OpenEditorRequest {
    pub(crate) editor_type: Option<String>,
    pub(crate) file_path: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct OpenEditorResponse {
    pub(crate) url: Option<String>,
}

pub(crate) async fn open_editor_http(
    base_url: &str,
    attempt_id: Uuid,
    file_path: Option<String>,
) -> anyhow::Result<Option<String>> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/open-editor"),
    );
    let body = OpenEditorRequest {
        editor_type: None,
        file_path,
    };
    let resp = client.post(endpoint).json(&body).send().await?;
    let api = decode_json_response::<ApiResponseWire<OpenEditorResponse>>(resp).await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected open-editor request")
        );
    }
    Ok(api.data.and_then(|d| d.url))
}
