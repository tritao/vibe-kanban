use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::state::DiffTheme;

use super::{diff_rows_with_all, highlight_unified_diff, DIFF_ALL_KEY};

fn hash_text_sample(hasher: &mut impl Hasher, s: &str) {
    s.len().hash(hasher);
    let bytes = s.as_bytes();
    let take = bytes.len().min(4096);
    bytes[..take].hash(hasher);
    if bytes.len() > take {
        bytes[bytes.len().saturating_sub(take)..].hash(hasher);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DiffPreviewItem {
    key: String,
    highlight_path: String,
    old: String,
    new: String,
    omitted: bool,
    adds: u64,
    dels: u64,
}

#[derive(Debug, Clone)]
pub(crate) enum DiffPreviewRequest {
    None,
    Single {
        key: String,
        highlight_path: String,
        old: String,
        new: String,
        omitted: bool,
        adds: u64,
        dels: u64,
    },
    All {
        items: Vec<DiffPreviewItem>,
    },
}

pub(crate) fn build_diff_preview_request(
    diff_store: &serde_json::Value,
    selected_diff_index: usize,
) -> DiffPreviewRequest {
    let rows = diff_rows_with_all(diff_store);
    let selected = rows
        .get(selected_diff_index.min(rows.len().saturating_sub(1)))
        .cloned();
    let Some(selected) = selected else {
        return DiffPreviewRequest::None;
    };

    let entries = diff_store.get("entries").and_then(|v| v.as_object());

    if selected.key == DIFF_ALL_KEY {
        let mut items: Vec<DiffPreviewItem> = vec![];
        for row in rows.iter().skip(1) {
            let Some(content) = entries
                .and_then(|e| e.get(&row.key))
                .and_then(|v| v.get("content"))
            else {
                continue;
            };

            let omitted = content
                .get("contentOmitted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let adds = content
                .get("additions")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let dels = content
                .get("deletions")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let old = content
                .get("oldContent")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let new = content
                .get("newContent")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let highlight_path = row
                .new_path
                .as_deref()
                .or(row.old_path.as_deref())
                .unwrap_or(&row.key)
                .to_string();
            items.push(DiffPreviewItem {
                key: row.key.clone(),
                highlight_path,
                old,
                new,
                omitted,
                adds,
                dels,
            });
        }
        return DiffPreviewRequest::All { items };
    }

    let content = entries
        .and_then(|e| e.get(&selected.key))
        .and_then(|v| v.get("content"));
    let Some(content) = content else {
        return DiffPreviewRequest::Single {
            key: selected.key.clone(),
            highlight_path: selected
                .new_path
                .as_deref()
                .or(selected.old_path.as_deref())
                .unwrap_or(&selected.key)
                .to_string(),
            old: String::new(),
            new: String::new(),
            omitted: false,
            adds: 0,
            dels: 0,
        };
    };

    let omitted = content
        .get("contentOmitted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let adds = content
        .get("additions")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let dels = content
        .get("deletions")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let old = content
        .get("oldContent")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let new = content
        .get("newContent")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let highlight_path = selected
        .new_path
        .as_deref()
        .or(selected.old_path.as_deref())
        .unwrap_or(&selected.key)
        .to_string();
    DiffPreviewRequest::Single {
        key: selected.key.clone(),
        highlight_path,
        old,
        new,
        omitted,
        adds,
        dels,
    }
}

pub(crate) fn compute_diff_preview(
    req: DiffPreviewRequest,
    width: usize,
    theme: DiffTheme,
    wrap: bool,
) -> (Option<String>, u64, Vec<Line<'static>>) {
    let mut hasher = DefaultHasher::new();
    theme.hash(&mut hasher);
    wrap.hash(&mut hasher);
    width.hash(&mut hasher);

    const MAX_DIFF_PREVIEW_LINES: usize = 20_000;

    match req {
        DiffPreviewRequest::None => (None, 0, vec![Line::from("No diffs")]),
        DiffPreviewRequest::Single {
            key,
            highlight_path,
            old,
            new,
            omitted,
            adds,
            dels,
        } => {
            key.hash(&mut hasher);
            omitted.hash(&mut hasher);
            adds.hash(&mut hasher);
            dels.hash(&mut hasher);
            hash_text_sample(&mut hasher, &old);
            hash_text_sample(&mut hasher, &new);
            let h = hasher.finish();

            if omitted {
                let lines = vec![Line::from(format!(
                    "{key} (content omitted)  +{adds}/-{dels}"
                ))];
                return (Some(key), h, lines);
            }

            let diff = utils::diff::create_unified_diff(&key, &old, &new);
            let mut lines = highlight_unified_diff(&highlight_path, &diff, width, theme, wrap);
            if lines.is_empty() {
                lines = vec![Line::from("No diff content")];
            }
            (Some(key), h, lines)
        }
        DiffPreviewRequest::All { items } => {
            DIFF_ALL_KEY.hash(&mut hasher);
            let mut lines: Vec<Line<'static>> = vec![];
            for (idx, item) in items.iter().enumerate() {
                item.key.hash(&mut hasher);
                item.omitted.hash(&mut hasher);
                item.adds.hash(&mut hasher);
                item.dels.hash(&mut hasher);
                hash_text_sample(&mut hasher, &item.old);
                hash_text_sample(&mut hasher, &item.new);

                if idx > 0 && !lines.is_empty() {
                    lines.push(Line::from(""));
                }

                if item.omitted {
                    lines.push(Line::from(format!(
                        "{} (content omitted)  +{}/-{}",
                        item.key, item.adds, item.dels
                    )));
                    continue;
                }
                let diff = utils::diff::create_unified_diff(&item.key, &item.old, &item.new);
                lines.extend(highlight_unified_diff(
                    &item.highlight_path,
                    &diff,
                    width,
                    theme,
                    wrap,
                ));
                if lines.len() > MAX_DIFF_PREVIEW_LINES {
                    lines.truncate(MAX_DIFF_PREVIEW_LINES);
                    lines.push(Line::from(Span::styled(
                        "… (truncated)".to_string(),
                        Style::default().add_modifier(Modifier::DIM),
                    )));
                    break;
                }
            }
            if lines.is_empty() {
                lines.push(Line::from("No diffs"));
            }
            (Some(DIFF_ALL_KEY.to_string()), hasher.finish(), lines)
        }
    }
}
