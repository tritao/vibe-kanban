use serde_json::Value;

#[derive(Debug, Clone)]
pub(crate) struct LogRoot {
    v: Value,
}

impl Default for LogRoot {
    fn default() -> Self {
        Self::empty()
    }
}

impl LogRoot {
    pub(crate) fn empty() -> Self {
        Self {
            v: serde_json::json!({ "entries": [] }),
        }
    }

    pub(crate) fn as_value(&self) -> &Value {
        &self.v
    }

    pub(crate) fn as_value_mut(&mut self) -> &mut Value {
        &mut self.v
    }

    pub(crate) fn ensure_entries_array(&mut self) {
        crate::store::log_patch::ensure_entries_array(&mut self.v);
    }

    pub(crate) fn entries_len(&self) -> usize {
        crate::store::log_patch::entries_len(&self.v)
    }

    pub(crate) fn entries(&self) -> &[Value] {
        self.v
            .get("entries")
            .and_then(|v| v.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub(crate) fn entries_mut(&mut self) -> &mut Vec<Value> {
        self.ensure_entries_array();
        crate::store::log_patch::entries_array_mut(&mut self.v)
    }
}
