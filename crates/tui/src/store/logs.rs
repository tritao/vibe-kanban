use serde_json::Value;

pub(crate) use super::log_entry::{EntryTypeRef, LogEntryKind, LogEntryRef, NormalizedContentRef};
use crate::logs::model_params::{ModelParams, parse_system_message_for_model_params};

pub(crate) struct LogStore<'a> {
    root: &'a Value,
}

impl<'a> LogStore<'a> {
    pub(crate) fn new(root: &'a Value) -> Self {
        Self { root }
    }

    pub(crate) fn model_params(&self) -> Option<ModelParams> {
        let entries = self.root.get("entries")?.as_array()?;
        for entry in entries {
            let entry = LogEntryRef::new(entry);
            if entry.kind() != LogEntryKind::NormalizedEntry {
                continue;
            }
            let content = entry.normalized_content()?;
            let entry_type = content.entry_type()?;
            if entry_type.tag() != "system_message" {
                continue;
            }
            let text = content.content_text();
            if let Some(p) = parse_system_message_for_model_params(text) {
                return Some(p);
            }
        }
        None
    }
}
