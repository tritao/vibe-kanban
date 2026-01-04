use anyhow::Context;
use uuid::Uuid;

use utils::response::ApiResponse;

use crate::state::{ConflictOp, MergeStatus, RepoBranchStatus, TaskStatus};
use crate::state::ExecutorProfileSelection;

pub(crate) async fn update_task_status_http(
    base_url: &str,
    task_id: Uuid,
    status: TaskStatus,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!("{}/api/tasks/{}", base_url.trim_end_matches('/'), task_id);
    let body = serde_json::json!({ "status": status.as_api_str() });

    let resp = client.put(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected status update");
    }
    Ok(())
}

pub(crate) async fn create_task_http(
    base_url: &str,
    project_id: Uuid,
    title: &str,
    description: Option<&str>,
    status: TaskStatus,
) -> anyhow::Result<Uuid> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!("{}/api/tasks", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "project_id": project_id,
        "title": title,
        "description": description,
        "status": status.as_api_str(),
        "parent_workspace_id": null,
        "image_ids": null,
        "shared_task_id": null,
    });

    #[derive(Debug, serde::Deserialize)]
    struct CreatedTaskDto {
        id: Uuid,
    }

    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<CreatedTaskDto>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected create task");
    }
    api.into_data()
        .map(|t| t.id)
        .ok_or_else(|| anyhow::anyhow!("missing task in create response"))
}

pub(crate) async fn stop_exec_http(base_url: &str, exec_id: Uuid) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/execution-processes/{}/stop",
        base_url.trim_end_matches('/'),
        exec_id
    );

    let resp = client.post(url).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
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
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/sessions?workspace_id={workspace_id}",
        base_url.trim_end_matches('/')
    );

    let resp = client.get(url).send().await?;
    let api = resp.json::<ApiResponse<Vec<SessionDto>>>().await?;
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
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/sessions/{session_id}/queue",
        base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({ "message": message, "variant": null });

    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected queue request");
    }
    Ok(())
}

pub(crate) async fn follow_up_http(
    base_url: &str,
    session_id: Uuid,
    prompt: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/sessions/{session_id}/follow-up",
        base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "prompt": prompt,
        "variant": null,
        "retry_process_id": null,
        "force_when_dirty": null,
        "perform_git_reset": null,
    });

    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected follow-up request");
    }
    Ok(())
}

pub(crate) async fn update_executor_profile_http(
    base_url: &str,
    profile: &ExecutorProfileSelection,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    // Fetch current config from /api/info so we can PUT the full config object.
    let info_url = format!("{}/api/info", base_url.trim_end_matches('/'));
    let resp = client.get(info_url).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
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

    let url = format!("{}/api/config", base_url.trim_end_matches('/'));
    let resp = client.put(url).json(&config).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected config update");
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ApiResponseWire<T, E = serde_json::Value> {
    pub(crate) success: bool,
    pub(crate) data: Option<T>,
    pub(crate) error_data: Option<E>,
    pub(crate) message: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct RepoIdRequest {
    pub(crate) repo_id: Uuid,
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

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PushErrorWire {
    ForcePushRequired,
}

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

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AttachPrResponse {
    pub(crate) pr_attached: bool,
    pub(crate) pr_url: Option<String>,
    #[allow(dead_code)]
    pub(crate) pr_number: Option<i64>,
    #[allow(dead_code)]
    pub(crate) pr_status: Option<MergeStatus>,
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

#[derive(Debug, serde::Serialize)]
pub(crate) struct OpenEditorRequest {
    pub(crate) editor_type: Option<String>,
    pub(crate) file_path: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct OpenEditorResponse {
    pub(crate) url: Option<String>,
}

pub(crate) async fn branch_status_http(
    base_url: &str,
    attempt_id: Uuid,
) -> anyhow::Result<Vec<RepoBranchStatus>> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/branch-status",
        base_url.trim_end_matches('/')
    );
    let resp = client.get(url).send().await?;
    let api = resp.json::<ApiResponseWire<Vec<RepoBranchStatus>>>().await?;
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

pub(crate) async fn rebase_task_attempt_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
    old_base_branch: Option<String>,
    new_base_branch: Option<String>,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/rebase",
        base_url.trim_end_matches('/')
    );
    let body = RebaseTaskAttemptRequest {
        repo_id,
        old_base_branch,
        new_base_branch,
    };

    let resp = client.post(url).json(&body).send().await?;
    let api = resp
        .json::<ApiResponseWire<serde_json::Value, GitOperationErrorWire>>()
        .await?;
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
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/conflicts/abort",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = resp.json::<ApiResponseWire<serde_json::Value>>().await?;
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
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/merge",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = resp.json::<ApiResponseWire<serde_json::Value>>().await?;
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

pub(crate) async fn push_task_attempt_branch_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/push",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = resp
        .json::<ApiResponseWire<serde_json::Value, PushErrorWire>>()
        .await?;
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
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/push/force",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = resp
        .json::<ApiResponseWire<serde_json::Value, PushErrorWire>>()
        .await?;
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

pub(crate) async fn create_pr_http(
    base_url: &str,
    attempt_id: Uuid,
    request: CreateGitHubPrRequest,
) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/pr",
        base_url.trim_end_matches('/')
    );
    let resp = client.post(url).json(&request).send().await?;
    let api = resp.json::<ApiResponseWire<String, CreatePrErrorWire>>().await?;
    if api.success {
        return Ok(api.data.unwrap_or_default());
    }
    if let Some(msg) = api.message {
        anyhow::bail!("{msg}");
    }
    if let Some(err) = api.error_data {
        let msg = match err {
            CreatePrErrorWire::GithubCliNotInstalled => "GitHub CLI (gh) not installed on the server",
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

pub(crate) async fn attach_pr_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<AttachPrResponse> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/pr/attach",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = resp.json::<ApiResponseWire<AttachPrResponse>>().await?;
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

pub(crate) async fn get_pr_comments_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<usize> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/pr/comments?repo_id={repo_id}",
        base_url.trim_end_matches('/')
    );
    let resp = client.get(url).send().await?;
    let api = resp
        .json::<ApiResponseWire<PrCommentsResponse, GetPrCommentsErrorWire>>()
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

pub(crate) async fn open_editor_http(
    base_url: &str,
    attempt_id: Uuid,
    file_path: Option<String>,
) -> anyhow::Result<Option<String>> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/open-editor",
        base_url.trim_end_matches('/')
    );
    let body = OpenEditorRequest {
        editor_type: None,
        file_path,
    };
    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponseWire<OpenEditorResponse>>().await?;
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
