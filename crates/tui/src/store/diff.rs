pub(crate) struct DiffStore<'a> {
    root: &'a serde_json::Value,
}

pub(crate) struct DiffEntryContent<'a> {
    root: &'a serde_json::Value,
}

impl<'a> DiffEntryContent<'a> {
    pub(crate) fn omitted(&self) -> bool {
        self.root
            .get("contentOmitted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    pub(crate) fn additions(&self) -> u64 {
        self.root
            .get("additions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    }

    pub(crate) fn deletions(&self) -> u64 {
        self.root
            .get("deletions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    }

    pub(crate) fn old_content(&self) -> &str {
        self.root
            .get("oldContent")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    }

    pub(crate) fn new_content(&self) -> &str {
        self.root
            .get("newContent")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    }
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

    pub(crate) fn entry_content_info(&self, key: &str) -> Option<DiffEntryContent<'a>> {
        Some(DiffEntryContent {
            root: self.entry_content(key)?,
        })
    }
}
