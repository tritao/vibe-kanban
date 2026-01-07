pub(crate) struct DiffStore<'a> {
    root: &'a serde_json::Value,
}

impl<'a> DiffStore<'a> {
    pub(crate) fn new(root: &'a serde_json::Value) -> Self {
        Self { root }
    }

    pub(crate) fn entries_object(&self) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
        self.root.get("entries")?.as_object()
    }

    pub(crate) fn has_entries(&self) -> bool {
        self.entries_object().is_some_and(|o| !o.is_empty())
    }
}
