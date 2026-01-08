use uuid::Uuid;

pub(crate) fn empty_projects_store() -> serde_json::Value {
    serde_json::json!({ "projects": {} })
}

pub(crate) struct ProjectsStore<'a> {
    root: &'a serde_json::Value,
}

impl<'a> ProjectsStore<'a> {
    pub(crate) fn new(root: &'a serde_json::Value) -> Self {
        Self { root }
    }

    pub(crate) fn project_name(&self, id: Uuid) -> Option<String> {
        let project = self.root.get("projects")?.get(id.to_string())?;
        let name = project
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| project.get("title").and_then(|v| v.as_str()))
            .unwrap_or("(unnamed)");
        Some(name.to_string())
    }

    pub(crate) fn projects_object(&self) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
        self.root.get("projects")?.as_object()
    }

    pub(crate) fn projects(&self) -> Vec<(Uuid, String)> {
        let Some(projects_obj) = self.projects_object() else {
            return vec![];
        };

        let mut rows = Vec::with_capacity(projects_obj.len());
        for (id_str, project) in projects_obj.iter() {
            let Ok(id) = Uuid::parse_str(id_str) else {
                continue;
            };
            let name = project
                .get("name")
                .and_then(|v| v.as_str())
                .or_else(|| project.get("title").and_then(|v| v.as_str()))
                .unwrap_or("(unnamed)")
                .to_string();
            rows.push((id, name));
        }

        rows.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        rows
    }
}
