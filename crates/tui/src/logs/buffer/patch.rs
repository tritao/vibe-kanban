fn ensure_log_store_entries_array(store: &mut serde_json::Value) {
    if !store.is_object() {
        *store = serde_json::json!({});
    }
    let obj = store.as_object_mut().expect("object");
    if !obj.get("entries").is_some_and(|v| v.is_array()) {
        obj.insert("entries".to_string(), serde_json::json!([]));
    }
}

fn parse_entries_index_and_suffix(path: &str) -> Option<(usize, &str)> {
    let rest = path.strip_prefix("/entries/")?;
    let (idx, suffix) = match rest.split_once('/') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };
    let i = idx.parse::<usize>().ok()?;
    let suffix = suffix.map(|s| {
        // Re-add leading slash so callers can reuse it as a JSON pointer suffix.
        // SAFETY: stored for the duration of this function call chain.
        Box::leak(format!("/{s}").into_boxed_str()) as &str
    });
    Some((i, suffix.unwrap_or("")))
}

pub(super) fn apply_log_patch_resilient(
    store: &mut serde_json::Value,
    patch: &json_patch::Patch,
) -> anyhow::Result<()> {
    ensure_log_store_entries_array(store);

    // Fast-path: json_patch is much faster, but it will "insert" again on reconnect when we
    // replay history (Add at an index that already exists). Detect that and use the resilient
    // idempotent path instead.
    let existing_len = store
        .get("entries")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let has_replay_add = patch.iter().any(|op| {
        if let json_patch::PatchOperation::Add(add) = op {
            let p = add.path.to_string();
            if let Some((idx, suffix)) = parse_entries_index_and_suffix(&p) {
                return suffix.is_empty() && idx < existing_len;
            }
        }
        false
    });

    if !has_replay_add && json_patch::patch(store, patch).is_ok() {
        return Ok(());
    }

    use json_patch::{AddOperation, PatchOperation, RemoveOperation, ReplaceOperation};

    for op in patch.iter() {
        match op {
            PatchOperation::Add(AddOperation { path, value }) => {
                let p = path.to_string();
                if let Some((idx, suffix)) = parse_entries_index_and_suffix(&p) {
                    ensure_log_store_entries_array(store);
                    let entries = store
                        .get_mut("entries")
                        .and_then(|v| v.as_array_mut())
                        .expect("entries array");
                    if suffix.is_empty() {
                        let v = value.clone();
                        // Idempotent semantics: if the index already exists (replay/reconnect),
                        // treat Add as Replace; otherwise append/insert.
                        if idx < entries.len() {
                            entries[idx] = v;
                        } else if idx == entries.len() {
                            entries.push(v);
                        } else {
                            entries.push(v);
                        }
                        continue;
                    }
                    if idx < entries.len() {
                        let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
                    }
                    continue;
                }
                let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
            }
            PatchOperation::Replace(ReplaceOperation { path, value }) => {
                let p = path.to_string();
                if let Some((idx, suffix)) = parse_entries_index_and_suffix(&p) {
                    ensure_log_store_entries_array(store);
                    let entries = store
                        .get_mut("entries")
                        .and_then(|v| v.as_array_mut())
                        .expect("entries array");
                    if suffix.is_empty() {
                        let v = value.clone();
                        if idx < entries.len() {
                            entries[idx] = v;
                        } else if idx == entries.len() {
                            entries.push(v);
                        } else {
                            // Missing history: ignore.
                        }
                        continue;
                    }
                    if idx < entries.len() {
                        let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
                    }
                    continue;
                }
                let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
            }
            PatchOperation::Remove(RemoveOperation { path }) => {
                let p = path.to_string();
                if let Some((idx, suffix)) = parse_entries_index_and_suffix(&p) {
                    ensure_log_store_entries_array(store);
                    let entries = store
                        .get_mut("entries")
                        .and_then(|v| v.as_array_mut())
                        .expect("entries array");
                    if suffix.is_empty() {
                        if idx < entries.len() {
                            entries.remove(idx);
                        }
                        continue;
                    }
                    if idx < entries.len() {
                        let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
                    }
                    continue;
                }
                let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
            }
            _ => {
                let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
            }
        }
    }

    Ok(())
}

pub(super) fn log_patch_min_entry_index(patch: &json_patch::Patch) -> Option<usize> {
    patch
        .iter()
        .filter_map(|op| {
            let path = op.path().to_string();
            let rest = path.strip_prefix("/entries/")?;
            let idx = rest.split('/').next()?;
            idx.parse::<usize>().ok()
        })
        .min()
}
