use serde_json::Value;

#[derive(Debug, Clone)]
pub(crate) struct ProjectsRoot {
    v: Value,
}

impl Default for ProjectsRoot {
    fn default() -> Self {
        Self::empty()
    }
}

impl ProjectsRoot {
    pub(crate) fn empty() -> Self {
        Self {
            v: serde_json::json!({ "projects": {} }),
        }
    }

    pub(crate) fn as_value(&self) -> &Value {
        &self.v
    }

    pub(crate) fn ensure_shape(&mut self) {
        if !self.v.is_object() {
            self.v = serde_json::json!({});
        }
        let obj = self.v.as_object_mut().expect("object");
        if !obj.get("projects").is_some_and(|v| v.is_object()) {
            obj.insert("projects".to_string(), serde_json::json!({}));
        }
    }

    pub(crate) fn apply_patch(
        &mut self,
        patch: &json_patch::Patch,
    ) -> Result<(), json_patch::PatchError> {
        self.ensure_shape();
        json_patch::patch(&mut self.v, patch)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TasksRoot {
    v: Value,
}

impl Default for TasksRoot {
    fn default() -> Self {
        Self::empty()
    }
}

impl TasksRoot {
    pub(crate) fn empty() -> Self {
        Self {
            v: serde_json::json!({ "tasks": {} }),
        }
    }

    pub(crate) fn as_value(&self) -> &Value {
        &self.v
    }

    pub(crate) fn ensure_shape(&mut self) {
        if !self.v.is_object() {
            self.v = serde_json::json!({});
        }
        let obj = self.v.as_object_mut().expect("object");
        if !obj.get("tasks").is_some_and(|v| v.is_object()) {
            obj.insert("tasks".to_string(), serde_json::json!({}));
        }
    }

    pub(crate) fn apply_patch(
        &mut self,
        patch: &json_patch::Patch,
    ) -> Result<(), json_patch::PatchError> {
        self.ensure_shape();
        json_patch::patch(&mut self.v, patch)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ExecRoot {
    v: Value,
}

impl Default for ExecRoot {
    fn default() -> Self {
        Self::empty()
    }
}

impl ExecRoot {
    pub(crate) fn empty() -> Self {
        Self {
            v: serde_json::json!({ "execution_processes": {} }),
        }
    }

    pub(crate) fn as_value(&self) -> &Value {
        &self.v
    }

    pub(crate) fn ensure_shape(&mut self) {
        if !self.v.is_object() {
            self.v = serde_json::json!({});
        }
        let obj = self.v.as_object_mut().expect("object");
        if !obj
            .get("execution_processes")
            .is_some_and(|v| v.is_object())
        {
            obj.insert("execution_processes".to_string(), serde_json::json!({}));
        }
    }

    pub(crate) fn apply_patch(
        &mut self,
        patch: &json_patch::Patch,
    ) -> Result<(), json_patch::PatchError> {
        self.ensure_shape();
        json_patch::patch(&mut self.v, patch)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DiffRoot {
    v: Value,
}

impl Default for DiffRoot {
    fn default() -> Self {
        Self::empty()
    }
}

impl DiffRoot {
    pub(crate) fn empty() -> Self {
        Self {
            v: serde_json::json!({ "entries": {} }),
        }
    }

    pub(crate) fn as_value(&self) -> &Value {
        &self.v
    }

    pub(crate) fn ensure_shape(&mut self) {
        if !self.v.is_object() {
            self.v = serde_json::json!({});
        }
        let obj = self.v.as_object_mut().expect("object");
        if !obj.get("entries").is_some_and(|v| v.is_object()) {
            obj.insert("entries".to_string(), serde_json::json!({}));
        }
    }

    pub(crate) fn apply_patch(
        &mut self,
        patch: &json_patch::Patch,
    ) -> Result<(), json_patch::PatchError> {
        self.ensure_shape();
        json_patch::patch(&mut self.v, patch)
    }
}
