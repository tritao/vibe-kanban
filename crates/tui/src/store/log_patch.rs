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

pub(crate) fn parse_entries_index_and_is_exact_entry(path: &str) -> Option<(usize, bool)> {
    let rest = path.strip_prefix("/entries/")?;
    let (idx, has_suffix) = match rest.split_once('/') {
        Some((a, _b)) => (a, true),
        None => (rest, false),
    };
    let i = idx.parse::<usize>().ok()?;
    Some((i, !has_suffix))
}
