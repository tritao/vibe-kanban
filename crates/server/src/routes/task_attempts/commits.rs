use std::path::Path;

use axum::{
    Router,
    extract::{Path as AxumPath, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::{repo::Repo, workspace_repo::WorkspaceRepo};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::{container::ContainerService, git::GitCli};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub struct CommitsQuery {
    pub repo_id: Uuid,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct CommitShowQuery {
    pub repo_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct CommitEntry {
    pub oid: String,
    pub short_oid: String,
    pub unix_ts: i64,
    pub subject: String,
}

#[derive(Debug, Serialize)]
pub struct CommitShowResponse {
    pub text: String,
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

async fn ensure_base_oid(
    deployment: &DeploymentImpl,
    workspace: &db::models::workspace::Workspace,
    repo: &Repo,
    workspace_repo: &db::models::workspace_repo::WorkspaceRepo,
) -> Result<String, ApiError> {
    let pool = &deployment.db().pool;
    if let Some(base) = WorkspaceRepo::get_diff_base_oid(pool, workspace.id, repo.id).await? {
        return Ok(base);
    }
    let base = deployment
        .git()
        .get_base_commit(&repo.path, &workspace.branch, &workspace_repo.target_branch)
        .map_err(ApiError::GitService)?;
    let _ =
        WorkspaceRepo::update_diff_base_oid(pool, workspace.id, repo.id, &base.to_string()).await;
    Ok(base.to_string())
}

fn parse_commit_log(out: &str) -> Vec<CommitEntry> {
    // Format: oid<US>short<US>ct<US>subject
    let mut commits = Vec::new();
    for line in out.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\x1f');
        let oid = parts.next().unwrap_or("").to_string();
        let short_oid = parts.next().unwrap_or("").to_string();
        let ts = parts
            .next()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let subject = parts.next().unwrap_or("").to_string();
        if oid.is_empty() {
            continue;
        }
        commits.push(CommitEntry {
            oid,
            short_oid,
            unix_ts: ts,
            subject,
        });
    }
    commits
}

pub async fn list_commits(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    Query(q): Query<CommitsQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<CommitEntry>>>, ApiError> {
    let pool = &deployment.db().pool;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let offset = q.offset.unwrap_or(0).min(50_000);

    let workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, q.repo_id)
            .await?
            .ok_or_else(|| ApiError::BadRequest("repo not found in this attempt".to_string()))?;

    let repo = Repo::find_by_id(pool, q.repo_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("repo not found".to_string()))?;

    let base_oid = ensure_base_oid(&deployment, &workspace, &repo, &workspace_repo).await?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;

    let range = format!("{base_oid}..HEAD");
    let offset_s = offset.to_string();
    let out = GitCli::new()
        .git(
            &worktree_path,
            [
                "--no-pager",
                "log",
                "--no-color",
                "--format=%H%x1f%h%x1f%ct%x1f%s",
                "--skip",
                &offset_s,
                "-n",
                &limit.to_string(),
                &range,
            ],
        )
        .map_err(|e| ApiError::BadRequest(format!("git log failed: {e}")))?;

    Ok(ResponseJson(ApiResponse::success(parse_commit_log(&out))))
}

pub async fn show_commit(
    axum::extract::Extension(workspace): axum::extract::Extension<db::models::workspace::Workspace>,
    State(deployment): State<DeploymentImpl>,
    AxumPath((_attempt_id, oid)): AxumPath<(Uuid, String)>,
    Query(q): Query<CommitShowQuery>,
) -> Result<ResponseJson<ApiResponse<CommitShowResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let _workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, q.repo_id)
            .await?
            .ok_or_else(|| ApiError::BadRequest("repo not found in this attempt".to_string()))?;
    let repo = Repo::find_by_id(pool, q.repo_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("repo not found".to_string()))?;
    let worktree_path = worktree_path_for_repo(&deployment, &workspace, &repo).await?;

    // Summary-only to keep rendering fast in the terminal.
    let out = GitCli::new()
        .git(
            &worktree_path,
            [
                "--no-pager",
                "show",
                "--no-color",
                "--name-status",
                "--format=commit %H%nAuthor: %an <%ae>%nDate:   %ad%n%n    %s%n",
                &oid,
            ],
        )
        .map_err(|e| ApiError::BadRequest(format!("git show failed: {e}")))?;

    Ok(ResponseJson(ApiResponse::success(CommitShowResponse {
        text: out,
    })))
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/", get(list_commits))
        .route("/{oid}", get(show_commit))
}
