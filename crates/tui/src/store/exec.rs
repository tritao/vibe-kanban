use uuid::Uuid;

use crate::state::{ExecRow, ExecStatus, RunReason, Timestamp};

pub(crate) fn empty_exec_store() -> serde_json::Value {
    serde_json::json!({ "execution_processes": {} })
}

pub(crate) struct ExecStore<'a> {
    root: &'a serde_json::Value,
}

impl<'a> ExecStore<'a> {
    pub(crate) fn new(root: &'a serde_json::Value) -> Self {
        Self { root }
    }

    pub(crate) fn execs_object(&self) -> Option<&'a serde_json::Map<String, serde_json::Value>> {
        self.root.get("execution_processes")?.as_object()
    }

    pub(crate) fn execs(&self) -> Vec<ExecRow> {
        let Some(exec_obj) = self.execs_object() else {
            return vec![];
        };

        let mut rows = Vec::with_capacity(exec_obj.len());
        for (id_str, exec) in exec_obj.iter() {
            let Ok(id) = Uuid::parse_str(id_str) else {
                continue;
            };
            let session_id = exec
                .get("session_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let run_reason = exec
                .get("run_reason")
                .and_then(|v| v.as_str())
                .map(RunReason::parse);
            let status = exec
                .get("status")
                .and_then(|v| v.as_str())
                .map(ExecStatus::parse);
            let created_at = exec
                .get("created_at")
                .and_then(|v| v.as_str())
                .and_then(Timestamp::parse);
            let dropped = exec
                .get("dropped")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            rows.push(ExecRow {
                id,
                session_id,
                run_reason,
                status,
                created_at,
                dropped,
            });
        }

        rows.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        rows
    }
}
