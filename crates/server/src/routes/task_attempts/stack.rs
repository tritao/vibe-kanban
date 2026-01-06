use std::path::Path;

use axum::{
    Router,
    extract::State,
    response::Json as ResponseJson,
    routing::{get, post},
};
use db::models::{repo::Repo, workspace_repo::WorkspaceRepo};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::{container::ContainerService, git::StgPatchState};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub struct StackRepoRequest {
    pub repo_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct StackNewPatchRequest {
    pub repo_id: Uuid,
    pub name: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct StackRefreshRequest {
    pub repo_id: Uuid,
    pub paths: Option<Vec<String>>,
    pub allow_dirty_index: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct StackRangeRequest {
    pub repo_id: Uuid,
    pub range: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StackGotoRequest {
    pub repo_id: Uuid,
    pub patch: String,
}

#[derive(Debug, Deserialize)]
pub struct StackFloatRequest {
    pub repo_id: Uuid,
    pub patches: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct StackRebaseRequest {
    pub repo_id: Uuid,
    pub new_base: String,
}

#[derive(Debug, Serialize)]
pub struct StackStatusResponse {
    pub enabled: bool,
    pub patches: Vec<PatchEntry>,
}

#[derive(Debug, Serialize)]
pub struct PatchEntry {
    pub name: String,
    pub description: Option<String>,
    pub state: String,
    pub is_current: bool,
}

async fn worktree_path_for_repo(
    deployment: &DeploymentImpl,
    workspace: &db::models::workspace::Workspace,
    repo: &Repo,
) -> Result<std::path::PathBuf, ApiError> {
    let container_ref = deployment
        .container()
        .ensure_container_exists(workspace)
        .await?;
    Ok(Path::new(&container_ref).join(&repo.name))
}

async fn load_repo(
    deployment: &DeploymentImpl,
    workspace: &db::models::workspace::Workspace,
    repo_id: Uuid,
) -> Result<Repo, ApiError> {
    let pool = &deployment.db().pool;
    let _workspace_repo = WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, repo_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("repo not found in this attempt".to_string()))?;

    Repo::find_by_id(pool, repo_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("repo not found".to_string()))
}

pub async fn status(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    axum::extract::Query(req): axum::extract::Query<StackRepoRequest>,
) -> Result<ResponseJson<ApiResponse<StackStatusResponse>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, req.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;

    let enabled = deployment
        .git()
        .stg_is_enabled(&worktree_path)
        .unwrap_or(false);
    let patches = if enabled {
        deployment
            .git()
            .stg_series(&worktree_path)
            .unwrap_or_default()
            .into_iter()
            .map(|p| PatchEntry {
                name: p.name,
                description: p.description,
                state: match p.state {
                    StgPatchState::Applied => "applied".to_string(),
                    StgPatchState::Unapplied => "unapplied".to_string(),
                },
                is_current: p.is_current,
            })
            .collect()
    } else {
        vec![]
    };

    Ok(ResponseJson(ApiResponse::success(StackStatusResponse {
        enabled,
        patches,
    })))
}

pub async fn enable(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<StackRepoRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, payload.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;
    deployment
        .git()
        .stg_enable(&worktree_path)
        .map_err(|e| ApiError::BadRequest(format!("stg init failed: {e}")))?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn new_patch(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<StackNewPatchRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, payload.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;
    deployment
        .git()
        .stg_new_patch(&worktree_path, payload.name.as_deref(), &payload.message)
        .map_err(|e| ApiError::BadRequest(format!("stg new failed: {e}")))?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn refresh(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<StackRefreshRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, payload.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;
    let paths = payload.paths.unwrap_or_default();
    let allow_dirty_index = payload.allow_dirty_index.unwrap_or(false);
    deployment
        .git()
        .stg_refresh(&worktree_path, &paths, allow_dirty_index)
        .map_err(|e| ApiError::BadRequest(format!("stg refresh failed: {e}")))?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn push(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<StackRangeRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, payload.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;
    deployment
        .git()
        .stg_push(&worktree_path, payload.range.as_deref())
        .map_err(|e| ApiError::BadRequest(format!("stg push failed: {e}")))?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn pop(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<StackRangeRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, payload.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;
    deployment
        .git()
        .stg_pop(&worktree_path, payload.range.as_deref())
        .map_err(|e| ApiError::BadRequest(format!("stg pop failed: {e}")))?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn goto(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<StackGotoRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, payload.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;
    deployment
        .git()
        .stg_goto(&worktree_path, &payload.patch)
        .map_err(|e| ApiError::BadRequest(format!("stg goto failed: {e}")))?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn float(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<StackFloatRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, payload.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;
    deployment
        .git()
        .stg_float(&worktree_path, &payload.patches)
        .map_err(|e| ApiError::BadRequest(format!("stg float failed: {e}")))?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn rebase(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<StackRebaseRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, payload.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;
    deployment
        .git()
        .stg_rebase(&worktree_path, &payload.new_base)
        .map_err(|e| ApiError::BadRequest(format!("stg rebase failed: {e}")))?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn undo(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<StackRepoRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, payload.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;
    deployment
        .git()
        .stg_undo(&worktree_path)
        .map_err(|e| ApiError::BadRequest(format!("stg undo failed: {e}")))?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn redo(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    ResponseJson(payload): ResponseJson<StackRepoRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let repo = load_repo(&deployment, &workspace, payload.repo_id).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;
    deployment
        .git()
        .stg_redo(&worktree_path)
        .map_err(|e| ApiError::BadRequest(format!("stg redo failed: {e}")))?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/status", get(status))
        .route("/enable", post(enable))
        .route("/new", post(new_patch))
        .route("/refresh", post(refresh))
        .route("/push", post(push))
        .route("/pop", post(pop))
        .route("/goto", post(goto))
        .route("/float", post(float))
        .route("/rebase", post(rebase))
        .route("/undo", post(undo))
        .route("/redo", post(redo))
}
