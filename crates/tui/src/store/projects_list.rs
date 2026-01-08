use uuid::Uuid;

#[derive(Debug, Clone)]
pub(crate) struct ProjectRow {
    pub(crate) id: Uuid,
    pub(crate) name: String,
}

pub(crate) fn projects_list(root: &serde_json::Value) -> Vec<ProjectRow> {
    super::projects::ProjectsStore::new(root)
        .projects()
        .into_iter()
        .map(|(id, name)| ProjectRow { id, name })
        .collect()
}
