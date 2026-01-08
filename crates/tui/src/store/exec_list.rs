use crate::state::ExecRow;

pub(crate) fn exec_list(root: &serde_json::Value) -> Vec<ExecRow> {
    super::exec::ExecStore::new(root).execs()
}
