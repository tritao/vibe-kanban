use serde_json::Value;

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

pub(crate) struct LogEntryRef<'a> {
    v: &'a Value,
}

impl<'a> LogEntryRef<'a> {
    pub(crate) fn new(v: &'a Value) -> Self {
        Self { v }
    }

    pub(crate) fn ty(&self) -> Option<&'a str> {
        self.v.get("type").and_then(|v| v.as_str())
    }

    pub(crate) fn stream_text(&self) -> Option<&'a str> {
        self.v.get("content").and_then(|v| v.as_str())
    }

    pub(crate) fn normalized_content(&self) -> Option<NormalizedContentRef<'a>> {
        if self.ty()? != "NORMALIZED_ENTRY" {
            return None;
        }
        Some(NormalizedContentRef::new(self.v.get("content")?))
    }
}

pub(crate) struct NormalizedContentRef<'a> {
    v: &'a Value,
}

impl<'a> NormalizedContentRef<'a> {
    pub(crate) fn new(v: &'a Value) -> Self {
        Self { v }
    }

    pub(crate) fn raw(&self) -> &'a Value {
        self.v
    }

    pub(crate) fn entry_type(&self) -> Option<EntryTypeRef<'a>> {
        Some(EntryTypeRef::new(self.v.get("entry_type")?))
    }

    pub(crate) fn content_text(&self) -> &'a str {
        self.v.get("content").and_then(|v| v.as_str()).unwrap_or("")
    }
}

pub(crate) struct EntryTypeRef<'a> {
    v: &'a Value,
}

impl<'a> EntryTypeRef<'a> {
    pub(crate) fn new(v: &'a Value) -> Self {
        Self { v }
    }

    pub(crate) fn raw(&self) -> &'a Value {
        self.v
    }

    pub(crate) fn tag(&self) -> &'a str {
        self.v
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    }

    pub(crate) fn denied_tool(&self) -> Option<&'a str> {
        self.v.get("denied_tool").and_then(|v| v.as_str())
    }

    pub(crate) fn action_type(&self) -> Option<ActionTypeRef<'a>> {
        Some(ActionTypeRef::new(self.v.get("action_type")?))
    }

    pub(crate) fn tool_status_str(&self) -> Option<&'a str> {
        let status = self.v.get("status")?;
        if let Some(s) = status.as_str() {
            return Some(s);
        }
        status.get("status").and_then(|v| v.as_str())
    }
}

pub(crate) struct ActionTypeRef<'a> {
    v: &'a Value,
}

impl<'a> ActionTypeRef<'a> {
    pub(crate) fn new(v: &'a Value) -> Self {
        Self { v }
    }

    pub(crate) fn action(&self) -> Option<&'a str> {
        self.v.get("action").and_then(|v| v.as_str())
    }

    pub(crate) fn result_output(&self) -> Option<&'a str> {
        self.v
            .get("result")
            .and_then(|v| v.get("output"))
            .and_then(|v| v.as_str())
    }

    pub(crate) fn changes(&self) -> Option<&'a Vec<Value>> {
        self.v.get("changes").and_then(|v| v.as_array())
    }

    pub(crate) fn unified_diffs_for_edit_actions(&self) -> Vec<&'a str> {
        let mut diffs: Vec<&'a str> = vec![];
        let Some(changes) = self.changes() else {
            return diffs;
        };
        for c in changes {
            if c.get("action").and_then(|v| v.as_str()) != Some("edit") {
                continue;
            }
            let diff = c.get("unified_diff").and_then(|v| v.as_str()).unwrap_or("");
            diffs.push(diff);
        }
        diffs
    }

    pub(crate) fn result_type(&self) -> Option<&'a str> {
        self.v
            .get("result")
            .and_then(|v| v.get("type"))
            .and_then(|v| v.as_str())
    }

    pub(crate) fn result_value(&self) -> Option<&'a Value> {
        self.v.get("result").and_then(|v| v.get("value"))
    }
}
