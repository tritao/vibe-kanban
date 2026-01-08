#[derive(Debug, Clone)]
pub(crate) struct DiffRow {
    pub(crate) key: String,
    pub(crate) change: Option<String>,
    pub(crate) additions: Option<usize>,
    pub(crate) deletions: Option<usize>,
    pub(crate) content_omitted: bool,
    pub(crate) old_path: Option<String>,
    pub(crate) new_path: Option<String>,
}

pub(crate) const DIFF_ALL_KEY: &str = "__ALL__";

fn diff_rows(store: &serde_json::Value) -> Vec<DiffRow> {
    let Some(entries) = store.get("entries").and_then(|v| v.as_object()) else {
        return vec![];
    };

    let mut rows = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        if value.get("type").and_then(|v| v.as_str()) != Some("DIFF") {
            continue;
        }
        let Some(content) = value.get("content") else {
            continue;
        };

        let change = content
            .get("change")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let additions = content
            .get("additions")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let deletions = content
            .get("deletions")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let content_omitted = content
            .get("contentOmitted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let old_path = content
            .get("oldPath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let new_path = content
            .get("newPath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        rows.push(DiffRow {
            key: key.clone(),
            change,
            additions,
            deletions,
            content_omitted,
            old_path,
            new_path,
        });
    }

    rows.sort_by(|a, b| a.key.cmp(&b.key));
    rows
}

pub(crate) fn diff_rows_filtered(store: &serde_json::Value, show_untracked: bool) -> Vec<DiffRow> {
    let mut rows = diff_rows(store);
    if !show_untracked {
        rows.retain(|r| r.change.as_deref() != Some("added"));
    }
    rows
}

pub(crate) fn diff_rows_with_all_filtered(
    store: &serde_json::Value,
    show_untracked: bool,
) -> Vec<DiffRow> {
    let mut rows = diff_rows_filtered(store, show_untracked);
    if rows.is_empty() {
        return rows;
    }

    let mut total_adds = 0usize;
    let mut total_dels = 0usize;
    let mut any_omitted = false;
    for row in &rows {
        total_adds = total_adds.saturating_add(row.additions.unwrap_or(0));
        total_dels = total_dels.saturating_add(row.deletions.unwrap_or(0));
        any_omitted |= row.content_omitted;
    }

    rows.insert(
        0,
        DiffRow {
            key: DIFF_ALL_KEY.to_string(),
            change: Some("all".to_string()),
            additions: Some(total_adds),
            deletions: Some(total_dels),
            content_omitted: any_omitted,
            old_path: None,
            new_path: None,
        },
    );
    rows
}
