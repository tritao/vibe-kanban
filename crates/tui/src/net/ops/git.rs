use uuid::Uuid;

use crate::{
    net::{
        api_client::{decode_api_response, decode_json_response, http_client, url},
        ops::{common::RepoIdRequest, wire::ApiResponseWire},
    },
    state::{ConflictOp, RepoBranchStatus},
};

pub(crate) async fn branch_status_http(
    base_url: &str,
    attempt_id: Uuid,
) -> anyhow::Result<Vec<RepoBranchStatus>> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/branch-status"),
    );
    let resp = client.get(endpoint).send().await?;
    let api = decode_json_response::<ApiResponseWire<Vec<RepoBranchStatus>>>(resp).await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected branch status request")
        );
    }
    Ok(api.data.unwrap_or_default())
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct RebaseTaskAttemptRequest {
    pub(crate) repo_id: Uuid,
    pub(crate) old_base_branch: Option<String>,
    pub(crate) new_base_branch: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum GitOperationErrorWire {
    MergeConflicts { message: String, _op: ConflictOp },
    RebaseInProgress,
}

pub(crate) async fn rebase_task_attempt_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
    old_base_branch: Option<String>,
    new_base_branch: Option<String>,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(base_url, &format!("/api/task-attempts/{attempt_id}/rebase"));
    let body = RebaseTaskAttemptRequest {
        repo_id,
        old_base_branch,
        new_base_branch,
    };

    let resp = client.post(endpoint).json(&body).send().await?;
    let api = decode_json_response::<ApiResponseWire<(), GitOperationErrorWire>>(resp).await?;
    if api.success {
        return Ok(());
    }

    if let Some(msg) = api.message {
        anyhow::bail!("{msg}");
    }
    if let Some(err) = api.error_data {
        match err {
            GitOperationErrorWire::MergeConflicts { message, .. } => anyhow::bail!("{message}"),
            GitOperationErrorWire::RebaseInProgress => anyhow::bail!("rebase already in progress"),
        }
    }
    anyhow::bail!("backend rejected rebase request");
}

pub(crate) async fn abort_conflicts_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/conflicts/abort"),
    );
    let resp = client
        .post(endpoint)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = decode_json_response::<ApiResponseWire<()>>(resp).await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected abort request")
        );
    }
    Ok(())
}

pub(crate) async fn merge_task_attempt_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(base_url, &format!("/api/task-attempts/{attempt_id}/merge"));
    let resp = client
        .post(endpoint)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = decode_json_response::<ApiResponseWire<()>>(resp).await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected merge request")
        );
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PushErrorWire {
    ForcePushRequired,
}

pub(crate) async fn push_task_attempt_branch_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(base_url, &format!("/api/task-attempts/{attempt_id}/push"));
    let resp = client
        .post(endpoint)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = decode_json_response::<ApiResponseWire<(), PushErrorWire>>(resp).await?;
    if api.success {
        return Ok(());
    }
    if let Some(PushErrorWire::ForcePushRequired) = api.error_data {
        anyhow::bail!("push rejected (use /push --force)");
    }
    anyhow::bail!(
        "{}",
        api.message
            .as_deref()
            .unwrap_or("backend rejected push request")
    );
}

pub(crate) async fn force_push_task_attempt_branch_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/push/force"),
    );
    let resp = client
        .post(endpoint)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = decode_json_response::<ApiResponseWire<(), PushErrorWire>>(resp).await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected force-push request")
        );
    }
    Ok(())
}

pub(crate) async fn repo_branches_http(
    base_url: &str,
    repo_id: Uuid,
) -> anyhow::Result<Vec<crate::state::GitBranchItem>> {
    let client = http_client()?;

    let endpoint = url(base_url, &format!("/api/repos/{repo_id}/branches"));
    let resp = client.get(endpoint).send().await?;
    let api = decode_api_response::<Vec<crate::state::GitBranchItem>>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected branches request");
    }
    Ok(api.into_data().unwrap_or_default())
}

pub(crate) async fn change_target_branch_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
    new_target_branch: &str,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/change-target-branch"),
    );
    let body = serde_json::json!({
        "repo_id": repo_id,
        "new_target_branch": new_target_branch,
    });
    let resp = client.post(endpoint).json(&body).send().await?;
    let api = decode_api_response::<()>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected change-target-branch");
    }
    Ok(())
}

pub(crate) async fn checkout_attempt_branch_http(
    base_url: &str,
    attempt_id: Uuid,
    branch: &str,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/checkout-branch"),
    );
    let body = serde_json::json!({
        "branch": branch,
        "force": false,
    });
    let resp = client.post(endpoint).json(&body).send().await?;
    let api = decode_api_response::<()>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected checkout-branch");
    }
    Ok(())
}
