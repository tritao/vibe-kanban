use uuid::Uuid;

use crate::{
    net::{
        api_client::{decode_json_response, http_client, url},
        ops::{common::RepoIdRequest, wire::ApiResponseWire},
    },
    state::MergeStatus,
};

#[derive(Debug, serde::Serialize)]
pub(crate) struct CreateGitHubPrRequest {
    pub(crate) title: String,
    pub(crate) body: Option<String>,
    pub(crate) target_branch: Option<String>,
    pub(crate) draft: Option<bool>,
    pub(crate) repo_id: Uuid,
    #[serde(default)]
    pub(crate) auto_generate_description: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CreatePrErrorWire {
    GithubCliNotInstalled,
    GithubCliNotLoggedIn,
    GitCliNotLoggedIn,
    GitCliNotInstalled,
    TargetBranchNotFound { branch: String },
}

pub(crate) async fn create_pr_http(
    base_url: &str,
    attempt_id: Uuid,
    request: CreateGitHubPrRequest,
) -> anyhow::Result<String> {
    let client = http_client()?;

    let endpoint = url(base_url, &format!("/api/task-attempts/{attempt_id}/pr"));
    let resp = client.post(endpoint).json(&request).send().await?;
    let api = decode_json_response::<ApiResponseWire<String, CreatePrErrorWire>>(resp).await?;
    if api.success {
        return Ok(api.data.unwrap_or_default());
    }
    if let Some(msg) = api.message {
        anyhow::bail!("{msg}");
    }
    if let Some(err) = api.error_data {
        let msg = match err {
            CreatePrErrorWire::GithubCliNotInstalled => {
                "GitHub CLI (gh) not installed on the server"
            }
            CreatePrErrorWire::GithubCliNotLoggedIn => "GitHub CLI (gh) not logged in",
            CreatePrErrorWire::GitCliNotLoggedIn => "git not authenticated (CLI auth failed)",
            CreatePrErrorWire::GitCliNotInstalled => "git not available on the server",
            CreatePrErrorWire::TargetBranchNotFound { branch } => {
                return Err(anyhow::anyhow!("target branch not found: {branch}"));
            }
        };
        anyhow::bail!("{msg}");
    }
    anyhow::bail!("backend rejected PR create request");
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AttachPrResponse {
    pub(crate) pr_attached: bool,
    pub(crate) pr_url: Option<String>,
    #[allow(dead_code)]
    pub(crate) pr_number: Option<i64>,
    #[allow(dead_code)]
    pub(crate) pr_status: Option<MergeStatus>,
}

pub(crate) async fn attach_pr_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<AttachPrResponse> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/pr/attach"),
    );
    let resp = client
        .post(endpoint)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = decode_json_response::<ApiResponseWire<AttachPrResponse>>(resp).await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected PR attach request")
        );
    }
    Ok(api.data.unwrap_or(AttachPrResponse {
        pr_attached: false,
        pr_url: None,
        pr_number: None,
        pr_status: None,
    }))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GetPrCommentsErrorWire {
    NoPrAttached,
    GithubCliNotInstalled,
    GithubCliNotLoggedIn,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct PrCommentsResponse {
    pub(crate) comments: Vec<serde_json::Value>,
}

pub(crate) async fn get_pr_comments_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<usize> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/pr/comments?repo_id={repo_id}"),
    );
    let resp = client.get(endpoint).send().await?;
    let api =
        decode_json_response::<ApiResponseWire<PrCommentsResponse, GetPrCommentsErrorWire>>(resp)
            .await?;
    if api.success {
        return Ok(api.data.map(|d| d.comments.len()).unwrap_or(0));
    }
    if let Some(msg) = api.message {
        anyhow::bail!("{msg}");
    }
    if let Some(err) = api.error_data {
        let msg = match err {
            GetPrCommentsErrorWire::NoPrAttached => "no PR attached",
            GetPrCommentsErrorWire::GithubCliNotInstalled => {
                "GitHub CLI (gh) not installed on the server"
            }
            GetPrCommentsErrorWire::GithubCliNotLoggedIn => "GitHub CLI (gh) not logged in",
        };
        anyhow::bail!("{msg}");
    }
    anyhow::bail!("backend rejected PR comments request");
}
