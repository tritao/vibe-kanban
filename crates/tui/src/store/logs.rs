use crate::logs::model_params::{ModelParams, parse_system_message_for_model_params};

pub(crate) struct LogStore<'a> {
    root: &'a serde_json::Value,
}

impl<'a> LogStore<'a> {
    pub(crate) fn new(root: &'a serde_json::Value) -> Self {
        Self { root }
    }

    pub(crate) fn model_params(&self) -> Option<ModelParams> {
        let entries = self.root.get("entries")?.as_array()?;
        for entry in entries {
            if entry.get("type")?.as_str()? != "NORMALIZED_ENTRY" {
                continue;
            }
            let content = entry.get("content")?;
            let entry_type = content.get("entry_type")?;
            if entry_type.get("type")?.as_str()? != "system_message" {
                continue;
            }
            let text = content.get("content")?.as_str().unwrap_or("");
            if let Some(p) = parse_system_message_for_model_params(text) {
                return Some(p);
            }
        }
        None
    }
}
