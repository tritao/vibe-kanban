#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiMessageKind {
    Notice,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiMessageKey {
    LogStreamConnect,
    BranchStatus,
    GitOp,
    CommitList,
    CommitPreview,
    StackOp,
    PullRequestOp,
}

#[derive(Debug, Clone)]
pub(crate) struct UiMessage {
    #[allow(dead_code)]
    pub(crate) kind: UiMessageKind,
    pub(crate) scope: Option<UiMessageKey>,
    pub(crate) text: String,
    #[allow(dead_code)]
    pub(crate) created_at: std::time::Instant,
}
