use crate::store::log_patch;

pub(super) fn apply_log_patch_resilient(
    store: &mut serde_json::Value,
    patch: &json_patch::Patch,
) -> anyhow::Result<()> {
    log_patch::ensure_entries_array(store);

    // Fast-path: json_patch is much faster, but it will "insert" again on reconnect when we
    // replay history (Add at an index that already exists). Detect that and use the resilient
    // idempotent path instead.
    let existing_len = log_patch::entries_len(store);
    let has_replay_add = patch.iter().any(|op| {
        if let json_patch::PatchOperation::Add(add) = op {
            let p = add.path.to_string();
            if let Some((idx, is_exact_entry)) =
                log_patch::parse_entries_index_and_is_exact_entry(&p)
            {
                return is_exact_entry && idx < existing_len;
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
                if let Some((idx, is_exact_entry)) =
                    log_patch::parse_entries_index_and_is_exact_entry(&p)
                {
                    log_patch::ensure_entries_array(store);
                    let entries = log_patch::entries_array_mut(store);
                    if is_exact_entry {
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
                if let Some((idx, is_exact_entry)) =
                    log_patch::parse_entries_index_and_is_exact_entry(&p)
                {
                    log_patch::ensure_entries_array(store);
                    let entries = log_patch::entries_array_mut(store);
                    if is_exact_entry {
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
                if let Some((idx, is_exact_entry)) =
                    log_patch::parse_entries_index_and_is_exact_entry(&p)
                {
                    log_patch::ensure_entries_array(store);
                    let entries = log_patch::entries_array_mut(store);
                    if is_exact_entry {
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
