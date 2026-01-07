#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NormalizedEntryType {
    UserMessage,
    AssistantMessage,
    SystemMessage,
    ErrorMessage,
    UserFeedback,
    Thinking,
    Loading,
    NextAction,
    ToolUse,
    Other,
}

impl NormalizedEntryType {
    pub(crate) fn parse(tag: &str) -> Self {
        match tag {
            "user_message" => Self::UserMessage,
            "assistant_message" => Self::AssistantMessage,
            "system_message" => Self::SystemMessage,
            "error_message" => Self::ErrorMessage,
            "user_feedback" => Self::UserFeedback,
            "thinking" => Self::Thinking,
            "loading" => Self::Loading,
            "next_action" => Self::NextAction,
            "tool_use" => Self::ToolUse,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolUseAction {
    FileRead,
    Search,
    FileEdit,
    CommandRun,
    WebFetch,
    TaskCreate,
    PlanPresentation,
    TodoManagement,
    Tool,
    Other,
}

impl ToolUseAction {
    pub(crate) fn parse(action: &str) -> Self {
        match action {
            "file_read" => Self::FileRead,
            "search" => Self::Search,
            "file_edit" => Self::FileEdit,
            "command_run" => Self::CommandRun,
            "web_fetch" => Self::WebFetch,
            "task_create" => Self::TaskCreate,
            "plan_presentation" => Self::PlanPresentation,
            "todo_management" => Self::TodoManagement,
            "tool" => Self::Tool,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolStatus {
    Success,
    Failed,
    Denied,
    PendingApproval,
    TimedOut,
    Other,
}

impl ToolStatus {
    pub(crate) fn parse(status: &str) -> Self {
        match status {
            "success" => Self::Success,
            "failed" => Self::Failed,
            "denied" => Self::Denied,
            "pending_approval" => Self::PendingApproval,
            "timed_out" => Self::TimedOut,
            _ => Self::Other,
        }
    }
}
