use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogEntryKind {
    Stdout,
    Stderr,
    NormalizedEntry,
    Other,
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

    pub(crate) fn kind(&self) -> LogEntryKind {
        match self.ty() {
            Some("STDOUT") => LogEntryKind::Stdout,
            Some("STDERR") => LogEntryKind::Stderr,
            Some("NORMALIZED_ENTRY") => LogEntryKind::NormalizedEntry,
            _ => LogEntryKind::Other,
        }
    }

    pub(crate) fn stream_text(&self) -> Option<&'a str> {
        self.v.get("content").and_then(|v| v.as_str())
    }

    pub(crate) fn normalized_content(&self) -> Option<NormalizedContentRef<'a>> {
        if self.kind() != LogEntryKind::NormalizedEntry {
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

    pub(crate) fn entry_type_tag(&self) -> &'a str {
        self.entry_type().map(|t| t.tag()).unwrap_or("unknown")
    }

    pub(crate) fn is_progress(&self) -> bool {
        matches!(self.entry_type_tag(), "thinking" | "loading")
    }

    pub(crate) fn content_text(&self) -> &'a str {
        self.v.get("content").and_then(|v| v.as_str()).unwrap_or("")
    }

    pub(crate) fn is_model_params_system_message(&self) -> bool {
        if self.entry_type_tag() != "system_message" {
            return false;
        }
        crate::logs::model_params::is_model_params_system_message(self.content_text().trim())
    }
}

pub(crate) struct EntryTypeRef<'a> {
    v: &'a Value,
}

impl<'a> EntryTypeRef<'a> {
    pub(crate) fn new(v: &'a Value) -> Self {
        Self { v }
    }

    #[allow(dead_code)]
    pub(crate) fn raw(&self) -> &'a Value {
        self.v
    }

    pub(crate) fn tag(&self) -> &'a str {
        self.v
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    }

    pub(crate) fn tool_use_summary(&self) -> Option<String> {
        if self.tag() != "tool_use" {
            return None;
        }
        let tool = self.tool_name().unwrap_or("tool");
        let status = self.tool_status_str().unwrap_or("created");
        Some(format!("{tool} ({status})"))
    }

    pub(crate) fn denied_tool(&self) -> Option<&'a str> {
        self.v.get("denied_tool").and_then(|v| v.as_str())
    }

    pub(crate) fn tool_name(&self) -> Option<&'a str> {
        self.v.get("tool_name").and_then(|v| v.as_str())
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

    pub(crate) fn next_action_failed(&self) -> bool {
        self.v
            .get("failed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    pub(crate) fn next_action_needs_setup(&self) -> bool {
        self.v
            .get("needs_setup")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    pub(crate) fn next_action_execution_processes(&self) -> u64 {
        self.v
            .get("execution_processes")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    }
}

pub(crate) struct ActionTypeRef<'a> {
    v: &'a Value,
}

impl<'a> ActionTypeRef<'a> {
    pub(crate) fn new(v: &'a Value) -> Self {
        Self { v }
    }

    pub(crate) fn raw(&self) -> &'a Value {
        self.v
    }

    pub(crate) fn action(&self) -> Option<&'a str> {
        self.v.get("action").and_then(|v| v.as_str())
    }

    pub(crate) fn path(&self) -> Option<&'a str> {
        self.v.get("path").and_then(|v| v.as_str())
    }

    pub(crate) fn command(&self) -> Option<&'a str> {
        self.v.get("command").and_then(|v| v.as_str())
    }

    pub(crate) fn query(&self) -> Option<&'a str> {
        self.v.get("query").and_then(|v| v.as_str())
    }

    pub(crate) fn result_exit_code(&self) -> Option<i64> {
        let es = self.v.get("result")?.get("exit_status")?;
        es.get("code")
            .and_then(|v| v.as_i64())
            .or_else(|| es.as_i64())
            .or_else(|| es.as_u64().map(|v| v as i64))
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
