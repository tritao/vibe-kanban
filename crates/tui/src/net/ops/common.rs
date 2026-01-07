use uuid::Uuid;

#[derive(Debug, serde::Serialize)]
pub(crate) struct RepoIdRequest {
    pub(crate) repo_id: Uuid,
}
