use uuid::Uuid;

use crate::{
    net::api_client::{decode_api_response, http_client, url},
    state::ExecutorProfileSelection,
};

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
    let client = http_client()?;

    let endpoint = url(base_url, &format!("/api/task-attempts?task_id={task_id}"));
    let resp = client.get(endpoint).send().await?;
    let api = decode_api_response::<Vec<WorkspaceDto>>(resp).await?;
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
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/projects/{project_id}/repositories"),
    );
    let resp = client.get(endpoint).send().await?;
    let api = decode_api_response::<Vec<ProjectRepoItem>>(resp).await?;
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
    let client = http_client()?;

    let endpoint = url(base_url, "/api/task-attempts");
    let body = serde_json::json!({
        "task_id": task_id,
        "executor_profile_id": executor_profile,
        "repos": repos.into_iter().map(|(repo_id, target_branch)| serde_json::json!({
            "repo_id": repo_id,
            "target_branch": target_branch,
        })).collect::<Vec<_>>(),
    });

    let resp = client.post(endpoint).json(&body).send().await?;
    let api = decode_api_response::<WorkspaceDto>(resp).await?;
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
