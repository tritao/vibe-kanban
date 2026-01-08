use uuid::Uuid;

use crate::{selection::exec_list, state::ExecStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum JobKey {
    DiffPreview,
    BranchStatus,
    BranchStatusAuto,
    StackStatus,
    CommitList,
    CommitPreview,
    LogPrewarm,
    OpenEditor,
    ExecutorProfile,
    ModelSettings,
    PullRequestCreate,
    PullRequestAttach,
    PullRequestComments,
    TaskDelete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingExecHook {
    pub(crate) exec_id: Option<Uuid>,
    pub(crate) prev_exec_id: Option<Uuid>,
    pub(crate) wait_new_exec: bool,
}

pub(crate) struct PendingExecHookUpdate {
    pub(crate) next: Option<PendingExecHook>,
    pub(crate) should_refresh: bool,
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

    pub(crate) fn on_exec_store_updated(
        mut self,
        exec_store: &serde_json::Value,
    ) -> PendingExecHookUpdate {
        if self.wait_new_exec {
            let mut execs = exec_list(exec_store);
            execs.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            if let Some(latest) = execs.last().map(|e| e.id) {
                if self.prev_exec_id != Some(latest) {
                    self.exec_id = Some(latest);
                    self.prev_exec_id = None;
                    self.wait_new_exec = false;
                }
            }
        }

        let Some(exec_id) = self.exec_id else {
            return PendingExecHookUpdate {
                next: Some(self),
                should_refresh: false,
            };
        };

        let execs = exec_list(exec_store);
        let is_running = execs
            .iter()
            .find(|e| e.id == exec_id)
            .is_some_and(|e| e.status == Some(ExecStatus::Running));
        if is_running {
            return PendingExecHookUpdate {
                next: Some(self),
                should_refresh: false,
            };
        }

        PendingExecHookUpdate {
            next: None,
            should_refresh: true,
        }
    }
}
