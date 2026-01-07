pub(crate) fn default_collapsed_for_log_entry(entry: &serde_json::Value) -> bool {
    const THRESHOLD_LINES: usize = 24;

    let entry = crate::store::logs::LogEntryRef::new(entry);
    let Some(content) = entry.normalized_content() else {
        return false;
    };
    let Some(entry_type) = content.entry_type() else {
        return false;
    };
    if entry_type.tag() != "tool_use" {
        return false;
    }
    let Some(action_type) = entry_type.action_type() else {
        return false;
    };
    let Some(action) = action_type.action() else {
        return false;
    };

    match action {
        "command_run" => {
            let output = action_type.result_output().unwrap_or("");
            output.lines().count() > THRESHOLD_LINES
        }
        "file_edit" => {
            let mut lines = 0usize;
            let changes = action_type.changes();
            for c in changes.into_iter().flatten() {
                if c.get("action").and_then(|v| v.as_str()) == Some("edit") {
                    let diff = c.get("unified_diff").and_then(|v| v.as_str()).unwrap_or("");
                    lines = lines.saturating_add(diff.lines().count());
                    if lines > THRESHOLD_LINES {
                        return true;
                    }
                }
            }
            false
        }
        "tool" => {
            let result_type = action_type.result_type();
            let value = action_type.result_value();
            match (result_type, value) {
                (Some("markdown"), Some(v)) => v
                    .as_str()
                    .unwrap_or("")
                    .lines()
                    .count()
                    .gt(&THRESHOLD_LINES),
                (Some("json"), Some(v)) => serde_json::to_string_pretty(v)
                    .ok()
                    .map(|s| s.lines().count() > THRESHOLD_LINES)
                    .unwrap_or(false),
                _ => false,
            }
        }
        _ => false,
    }
}
