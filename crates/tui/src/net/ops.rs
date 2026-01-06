use anyhow::Context;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::state::{
    ConflictOp, ExecutorProfileSelection, MergeStatus, RepoBranchStatus, TaskStatus,
};

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
    parent_task_id: Option<Uuid>,
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
        "parent_task_id": parent_task_id,
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

#[derive(Debug, serde::Deserialize)]
struct WorkspaceDto {
    id: Uuid,
    branch: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    setup_completed_at: Option<String>,
}

pub(crate) async fn list_task_attempts_http(
    base_url: &str,
    task_id: Uuid,
) -> anyhow::Result<Vec<crate::state::AttemptRow>> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts?task_id={task_id}",
        base_url.trim_end_matches('/')
    );
    let resp = client.get(url).send().await?;
    let api = resp.json::<ApiResponse<Vec<WorkspaceDto>>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected task attempts request");
    }
    Ok(api
        .into_data()
        .unwrap_or_default()
        .into_iter()
        .map(|w| crate::state::AttemptRow {
            id: w.id,
            branch: w.branch,
            created_at: w.created_at,
            updated_at: w.updated_at,
            setup_completed_at: w.setup_completed_at,
        })
        .collect())
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ProjectRepoItem {
    pub(crate) id: Uuid,
    #[allow(dead_code)]
    pub(crate) name: String,
    #[allow(dead_code)]
    pub(crate) display_name: String,
}

pub(crate) async fn project_repositories_http(
    base_url: &str,
    project_id: Uuid,
) -> anyhow::Result<Vec<ProjectRepoItem>> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/projects/{project_id}/repositories",
        base_url.trim_end_matches('/')
    );
    let resp = client.get(url).send().await?;
    let api = resp.json::<ApiResponse<Vec<ProjectRepoItem>>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected project repositories request");
    }
    Ok(api.into_data().unwrap_or_default())
}

pub(crate) async fn create_task_attempt_http(
    base_url: &str,
    task_id: Uuid,
    executor_profile: &ExecutorProfileSelection,
    repos: Vec<(Uuid, String)>,
) -> anyhow::Result<crate::state::AttemptRow> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!("{}/api/task-attempts", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "task_id": task_id,
        "executor_profile_id": executor_profile,
        "repos": repos.into_iter().map(|(repo_id, target_branch)| serde_json::json!({
            "repo_id": repo_id,
            "target_branch": target_branch,
        })).collect::<Vec<_>>(),
    });

    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<WorkspaceDto>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected create task attempt");
    }
    let w = api
        .into_data()
        .ok_or_else(|| anyhow::anyhow!("missing workspace in create attempt response"))?;
    Ok(crate::state::AttemptRow {
        id: w.id,
        branch: w.branch,
        created_at: w.created_at,
        updated_at: w.updated_at,
        setup_completed_at: w.setup_completed_at,
    })
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
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!("{}/api/profiles", base_url.trim_end_matches('/'));
    let resp = client.get(url).send().await?;
    let api = resp.json::<ApiResponse<ProfilesContentDto>>().await?;
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

    let url = format!("{}/api/profiles", base_url.trim_end_matches('/'));
    let resp = client
        .put(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected profiles update");
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
    let api = resp
        .json::<ApiResponseWire<Vec<RepoBranchStatus>>>()
        .await?;
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
    let api = resp
        .json::<ApiResponseWire<String, CreatePrErrorWire>>()
        .await?;
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

pub(crate) async fn create_project_http(
    base_url: &str,
    name: &str,
    repo_path: &str,
    display_name: &str,
) -> anyhow::Result<Uuid> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!("{}/api/projects", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "name": name,
        "repositories": [
            {
                "display_name": display_name,
                "git_repo_path": repo_path,
            }
        ],
    });

    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected project create");
    }
    let proj = api
        .into_data()
        .ok_or_else(|| anyhow::anyhow!("missing project in create response"))?;
    let id = proj
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| anyhow::anyhow!("missing project id in response"))?;
    Ok(id)
}

pub(crate) async fn add_project_repository_http(
    base_url: &str,
    project_id: Uuid,
    repo_path: &str,
    display_name: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/projects/{project_id}/repositories",
        base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "display_name": display_name,
        "git_repo_path": repo_path,
    });

    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected add repository");
    }
    Ok(())
}

pub(crate) async fn find_project_for_repo_path_http(
    base_url: &str,
    repo_path: &str,
) -> anyhow::Result<Option<Uuid>> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let target = std::fs::canonicalize(repo_path)
        .unwrap_or_else(|_| std::path::PathBuf::from(repo_path))
        .to_string_lossy()
        .to_string();

    let url = format!("{}/api/projects", base_url.trim_end_matches('/'));
    let resp = client.get(url).send().await?;
    let api = resp.json::<ApiResponse<Vec<serde_json::Value>>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected projects request");
    }
    let projects = api.into_data().unwrap_or_default();

    for p in projects {
        let Some(id_str) = p.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Ok(project_id) = Uuid::parse_str(id_str) else {
            continue;
        };

        let url = format!(
            "{}/api/projects/{project_id}/repositories",
            base_url.trim_end_matches('/')
        );
        let resp = client.get(url).send().await?;
        let api = resp.json::<ApiResponse<Vec<serde_json::Value>>>().await?;
        if !api.is_success() {
            continue;
        }
        let repos = api.into_data().unwrap_or_default();
        for r in repos {
            if let Some(path_str) = r.get("path").and_then(|v| v.as_str()) {
                let p2 = std::fs::canonicalize(path_str)
                    .unwrap_or_else(|_| std::path::PathBuf::from(path_str))
                    .to_string_lossy()
                    .to_string();
                if p2 == target {
                    return Ok(Some(project_id));
                }
            }
        }
    }

    Ok(None)
}

pub(crate) async fn repo_branches_http(
    base_url: &str,
    repo_id: Uuid,
) -> anyhow::Result<Vec<crate::state::GitBranchItem>> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/repos/{repo_id}/branches",
        base_url.trim_end_matches('/')
    );
    let resp = client.get(url).send().await?;
    let api = resp
        .json::<ApiResponse<Vec<crate::state::GitBranchItem>>>()
        .await?;
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
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/change-target-branch",
        base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "repo_id": repo_id,
        "new_target_branch": new_target_branch,
    });
    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
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
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/checkout-branch",
        base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "branch": branch,
        "force": false,
    });
    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected checkout-branch");
    }
    Ok(())
}
