use serde_json::Value;

pub(crate) fn ensure_entries_array(store: &mut Value) {
    if !store.is_object() {
        *store = serde_json::json!({});
    }
    let obj = store.as_object_mut().expect("object");
    if !obj.get("entries").is_some_and(|v| v.is_array()) {
        obj.insert("entries".to_string(), serde_json::json!([]));
    }
}

pub(crate) fn entries_len(store: &Value) -> usize {
    store
        .get("entries")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

pub(crate) fn entries_array_mut(store: &mut Value) -> &mut Vec<Value> {
    store
        .get_mut("entries")
        .and_then(|v| v.as_array_mut())
        .expect("entries array")
}

pub(crate) fn parse_entries_index_and_suffix(path: &str) -> Option<(usize, &str)> {
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
