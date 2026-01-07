pub(crate) fn default_collapsed_for_log_entry(entry: &serde_json::Value) -> bool {
    const THRESHOLD_LINES: usize = 24;

    let Some(ty) = entry.get("type").and_then(|v| v.as_str()) else {
        return false;
    };
    if ty != "NORMALIZED_ENTRY" {
        return false;
    }

    let Some(content) = entry.get("content") else {
        return false;
    };
    let Some(entry_type) = content.get("entry_type") else {
        return false;
    };
    let Some(entry_type_tag) = entry_type.get("type").and_then(|v| v.as_str()) else {
        return false;
    };
    if entry_type_tag != "tool_use" {
        return false;
    }

    let Some(action_type) = entry_type.get("action_type") else {
        return false;
    };
    let Some(action) = action_type.get("action").and_then(|v| v.as_str()) else {
        return false;
    };

    match action {
        "command_run" => {
            let output = action_type
                .get("result")
                .and_then(|v| v.get("output"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            output.lines().count() > THRESHOLD_LINES
        }
        "file_edit" => {
            let mut lines = 0usize;
            let changes = action_type.get("changes").and_then(|v| v.as_array());
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
            let result_type = action_type
                .get("result")
                .and_then(|v| v.get("type"))
                .and_then(|v| v.as_str());
            let value = action_type.get("result").and_then(|v| v.get("value"));
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
