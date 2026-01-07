use serde_json::Value;

use crate::state::ExecutorProfileSelection;

pub(crate) struct ExecutorProfilesStore<'a> {
    root: &'a Value,
}

impl<'a> ExecutorProfilesStore<'a> {
    pub(crate) fn new(root: &'a Value) -> Self {
        Self { root }
    }

    pub(crate) fn resolve_executor_key<'b>(
        &'b self,
        selection: Option<&'b ExecutorProfileSelection>,
    ) -> Option<&'b str> {
        let selection = selection?;
        let execs = self.root.as_object()?;
        execs
            .keys()
            .find(|k| k.eq_ignore_ascii_case(&selection.executor))
            .map(|s| s.as_str())
            .or(Some(selection.executor.as_str()))
    }

    pub(crate) fn models_for_selected_executor(
        &self,
        selection: Option<&ExecutorProfileSelection>,
    ) -> Vec<String> {
        let Some(exec_key) = self.resolve_executor_key(selection) else {
            return vec![];
        };
        self.models_for_executor_key(exec_key)
    }

    pub(crate) fn current_model_and_effort(
        &self,
        selection: &ExecutorProfileSelection,
    ) -> (Option<String>, Option<String>) {
        let Some(execs) = self.root.as_object() else {
            return (None, None);
        };
        let exec_key = execs
            .keys()
            .find(|k| k.eq_ignore_ascii_case(&selection.executor))
            .cloned()
            .unwrap_or_else(|| selection.executor.clone());
        let Some(variants) = execs.get(&exec_key).and_then(|v| v.as_object()) else {
            return (None, None);
        };
        let wanted_variant = selection.variant.as_deref().unwrap_or("DEFAULT");
        let variant_key = variants
            .keys()
            .find(|k| k.eq_ignore_ascii_case(wanted_variant))
            .cloned()
            .unwrap_or_else(|| wanted_variant.to_string());
        let Some(variant) = variants.get(&variant_key).and_then(|v| v.as_object()) else {
            return (None, None);
        };
        let nested_key = variant
            .keys()
            .find(|k| k.eq_ignore_ascii_case(&exec_key))
            .cloned()
            .unwrap_or_else(|| exec_key.clone());
        let Some(cfg) = variant.get(&nested_key).and_then(|v| v.as_object()) else {
            return (None, None);
        };

        let model = cfg
            .get("model")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        let effort = cfg
            .get("model_reasoning_effort")
            .or_else(|| cfg.get("reasoning_effort"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        (model, effort)
    }

    fn models_for_executor_key(&self, exec_key: &str) -> Vec<String> {
        let Some(execs) = self.root.as_object() else {
            return vec![];
        };
        let Some(variants) = execs.get(exec_key).and_then(|v| v.as_object()) else {
            return vec![];
        };

        let mut models: std::collections::HashSet<String> = std::collections::HashSet::new();
        for variant in variants.values() {
            let Some(vobj) = variant.as_object() else {
                continue;
            };
            let nested_key = vobj
                .keys()
                .find(|k| k.eq_ignore_ascii_case(exec_key))
                .cloned();
            let Some(nested_key) = nested_key else {
                continue;
            };
            let Some(cfg) = vobj.get(&nested_key).and_then(|v| v.as_object()) else {
                continue;
            };
            if let Some(m) = cfg.get("model").and_then(|v| v.as_str()) {
                if !m.trim().is_empty() {
                    models.insert(m.to_string());
                }
            }
        }

        let mut out: Vec<String> = models.into_iter().collect();
        out.sort();
        out
    }
}
