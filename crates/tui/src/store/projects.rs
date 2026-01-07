use uuid::Uuid;

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
}
