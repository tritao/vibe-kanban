use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum JobKey {
    DiffPreview,
    BranchStatus,
    BranchStatusAuto,
    StackStatus,
    CommitList,
    CommitPreview,
    LogPrewarm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingExecHook {
    pub(crate) exec_id: Option<Uuid>,
    pub(crate) prev_exec_id: Option<Uuid>,
    pub(crate) wait_new_exec: bool,
}

impl PendingExecHook {
    pub(crate) fn for_exec(exec_id: Uuid) -> Self {
        Self {
            exec_id: Some(exec_id),
            prev_exec_id: None,
            wait_new_exec: false,
        }
    }

    pub(crate) fn after_next_exec(prev_exec_id: Option<Uuid>) -> Self {
        Self {
            exec_id: None,
            prev_exec_id,
            wait_new_exec: true,
        }
    }
}
