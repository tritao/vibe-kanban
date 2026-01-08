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

    pub(crate) fn rows_with_all_filtered(
        &self,
        show_untracked: bool,
    ) -> Vec<crate::store::diff_rows::DiffRow> {
        crate::store::diff_rows::diff_rows_with_all_filtered(self.root, show_untracked)
    }

    pub(crate) fn entry_content(&self, key: &str) -> Option<&'a serde_json::Value> {
        self.entries_object()?.get(key)?.get("content")
    }
}
