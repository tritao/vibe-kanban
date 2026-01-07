use uuid::Uuid;

use crate::{
    net::{
        api_client::{decode_json_response, http_client, url},
        ops::{common::RepoIdRequest, wire::ApiResponseWire},
    },
    state::{StackPatchEntry, StackStatusResponse},
};

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum StackErrorWire {
    StgNotInstalled,
    NotEnabled,
    ConflictsInProgress {
        message: String,
        op: Option<String>,
        files: Vec<String>,
    },
    DirtyWorktree {
        message: String,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct StackPatchWire {
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    pub(crate) state: String,
    pub(crate) is_current: bool,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct StackStatusWire {
    pub(crate) available: bool,
    pub(crate) enabled: bool,
    pub(crate) patches: Vec<StackPatchWire>,
}

fn map_stack_status_wire(dto: StackStatusWire) -> StackStatusResponse {
    StackStatusResponse {
        available: dto.available,
        enabled: dto.enabled,
        patches: dto
            .patches
            .into_iter()
            .map(|p| StackPatchEntry {
                name: p.name,
                description: p.description,
                state: p.state,
                is_current: p.is_current,
            })
            .collect(),
    }
}

fn stack_error_to_anyhow(err: StackErrorWire) -> anyhow::Error {
    match err {
        StackErrorWire::StgNotInstalled => {
            anyhow::anyhow!("stg is not installed on the backend host")
        }
        StackErrorWire::NotEnabled => anyhow::anyhow!("stack not enabled (run /stack enable)"),
        StackErrorWire::ConflictsInProgress { message, op, files } => {
            let mut msg = message;
            if let Some(op) = op {
                msg.push_str(&format!(" (op: {op})"));
            }
            if !files.is_empty() {
                msg.push_str(&format!("; files: {}", files.join(", ")));
            }
            anyhow::anyhow!("{msg}")
        }
        StackErrorWire::DirtyWorktree { message } => anyhow::anyhow!("{message}"),
        StackErrorWire::Failed { message } => anyhow::anyhow!("{message}"),
    }
}

fn stack_api_to_result(
    api: ApiResponseWire<StackStatusWire, StackErrorWire>,
    default_error: &str,
) -> anyhow::Result<StackStatusResponse> {
    if api.success {
        return Ok(map_stack_status_wire(
            api.data
                .ok_or_else(|| anyhow::anyhow!("missing stack status payload"))?,
        ));
    }
    if let Some(err) = api.error_data {
        return Err(stack_error_to_anyhow(err));
    }
    anyhow::bail!("{}", api.message.as_deref().unwrap_or(default_error));
}

pub(crate) async fn stack_status_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<StackStatusResponse> {
    let client = http_client()?;
    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/stack/status?repo_id={repo_id}"),
    );
    let resp = client.get(endpoint).send().await?;
    let api =
        decode_json_response::<ApiResponseWire<StackStatusWire, StackErrorWire>>(resp).await?;
    stack_api_to_result(api, "backend rejected stack status request")
}

pub(crate) async fn stack_enable_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<StackStatusResponse> {
    let client = http_client()?;
    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/stack/enable"),
    );
    let resp = client
        .post(endpoint)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api =
        decode_json_response::<ApiResponseWire<StackStatusWire, StackErrorWire>>(resp).await?;
    stack_api_to_result(api, "backend rejected stack enable request")
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct StackDisableRequest {
    pub(crate) repo_id: Uuid,
    pub(crate) force: Option<bool>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct StackNewPatchRequest {
    pub(crate) repo_id: Uuid,
    pub(crate) name: Option<String>,
    pub(crate) message: String,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct StackRefreshRequest {
    pub(crate) repo_id: Uuid,
    pub(crate) paths: Option<Vec<String>>,
    pub(crate) allow_dirty_index: Option<bool>,
}

async fn stack_post_repo_id(
    base_url: &str,
    attempt_id: Uuid,
    path: &str,
    repo_id: Uuid,
) -> anyhow::Result<StackStatusResponse> {
    let client = http_client()?;
    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/stack/{path}"),
    );
    let resp = client
        .post(endpoint)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api =
        decode_json_response::<ApiResponseWire<StackStatusWire, StackErrorWire>>(resp).await?;
    stack_api_to_result(api, "backend rejected stack request")
}

async fn stack_post_json<T: serde::Serialize>(
    base_url: &str,
    attempt_id: Uuid,
    path: &str,
    body: &T,
) -> anyhow::Result<StackStatusResponse> {
    let client = http_client()?;
    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/stack/{path}"),
    );
    let resp = client.post(endpoint).json(body).send().await?;
    let api =
        decode_json_response::<ApiResponseWire<StackStatusWire, StackErrorWire>>(resp).await?;
    stack_api_to_result(api, "backend rejected stack request")
}

pub(crate) async fn stack_push_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<StackStatusResponse> {
    stack_post_repo_id(base_url, attempt_id, "push", repo_id).await
}

pub(crate) async fn stack_disable_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
    force: bool,
) -> anyhow::Result<StackStatusResponse> {
    let body = StackDisableRequest {
        repo_id,
        force: Some(force),
    };
    stack_post_json(base_url, attempt_id, "disable", &body).await
}

pub(crate) async fn stack_pop_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<StackStatusResponse> {
    stack_post_repo_id(base_url, attempt_id, "pop", repo_id).await
}

pub(crate) async fn stack_undo_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<StackStatusResponse> {
    stack_post_repo_id(base_url, attempt_id, "undo", repo_id).await
}

pub(crate) async fn stack_redo_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<StackStatusResponse> {
    stack_post_repo_id(base_url, attempt_id, "redo", repo_id).await
}

pub(crate) async fn stack_new_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
    name: Option<String>,
    message: String,
) -> anyhow::Result<StackStatusResponse> {
    let body = StackNewPatchRequest {
        repo_id,
        name,
        message,
    };
    stack_post_json(base_url, attempt_id, "new", &body).await
}

pub(crate) async fn stack_refresh_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
    paths: Option<Vec<String>>,
    allow_dirty_index: bool,
) -> anyhow::Result<StackStatusResponse> {
    let body = StackRefreshRequest {
        repo_id,
        paths,
        allow_dirty_index: Some(allow_dirty_index),
    };
    stack_post_json(base_url, attempt_id, "refresh", &body).await
}
