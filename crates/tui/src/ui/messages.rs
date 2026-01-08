pub(crate) mod notices {
    pub(crate) const HELP_OPENED: &str = "Opened help. (Press Esc to close)";

    pub(crate) const COMMITS_REFRESHING: &str = "Commits: refreshing…";
    pub(crate) const COMMITS_LOADED: &str = "Commits: loaded.";

    pub(crate) const STACK_REFRESHING: &str = "Stack: refreshing…";

    pub(crate) fn stack_enabling(repo_name: &str) -> String {
        format!("Stack: enabling for {repo_name}…")
    }

    pub(crate) fn stack_disabling(repo_name: &str, force: bool) -> String {
        format!(
            "Stack: disabling for {repo_name}{}…",
            if force { " (force)" } else { "" }
        )
    }

    pub(crate) fn stack_new(repo_name: &str) -> String {
        format!("Stack: new ({repo_name})…")
    }

    pub(crate) fn stack_refresh(repo_name: &str) -> String {
        format!("Stack: refresh ({repo_name})…")
    }

    pub(crate) fn stack_push(repo_name: &str) -> String {
        format!("Stack: push ({repo_name})…")
    }

    pub(crate) fn stack_pop(repo_name: &str) -> String {
        format!("Stack: pop ({repo_name})…")
    }

    pub(crate) fn stack_undo(repo_name: &str) -> String {
        format!("Stack: undo ({repo_name})…")
    }

    pub(crate) fn stack_redo(repo_name: &str) -> String {
        format!("Stack: redo ({repo_name})…")
    }
}

pub(crate) mod errors {
    pub(crate) const NO_TASK_SELECTED: &str = "no task selected";
    pub(crate) const NO_REPO_SELECTED: &str = "no repo selected";
    pub(crate) const NO_ATTEMPT_SELECTED: &str = "no attempt selected";
    pub(crate) const NO_REPO_STATUS_LOADED: &str = "no repo status loaded yet (run /status)";
    #[allow(dead_code)]
    pub(crate) const NO_EXECUTOR_SELECTED: &str = "no executor selected (try /executor)";
}
