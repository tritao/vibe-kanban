use uuid::Uuid;

use super::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    Todo,
    InProgress,
    InReview,
    Done,
    Cancelled,
}

impl TaskStatus {
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "todo" => Some(Self::Todo),
            "inprogress" => Some(Self::InProgress),
            "inreview" => Some(Self::InReview),
            "done" => Some(Self::Done),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub(crate) fn as_api_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "inprogress",
            Self::InReview => "inreview",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Todo => "Todo",
            Self::InProgress => "In Progress",
            Self::InReview => "In Review",
            Self::Done => "Done",
            Self::Cancelled => "Cancelled",
        }
    }

    pub(crate) fn idx(self) -> usize {
        match self {
            Self::Todo => 0,
            Self::InProgress => 1,
            Self::InReview => 2,
            Self::Done => 3,
            Self::Cancelled => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TaskRow {
    pub(crate) id: Uuid,
    pub(crate) title: String,
    pub(crate) status: TaskStatus,
    pub(crate) parent_task_id: Option<Uuid>,
    pub(crate) updated_at: Option<Timestamp>,
    pub(crate) has_in_progress_attempt: bool,
    pub(crate) last_attempt_failed: bool,
    pub(crate) executor: Option<String>,
    #[allow(dead_code)]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AttemptRow {
    pub(crate) id: Uuid,
    pub(crate) branch: String,
    #[allow(dead_code)]
    pub(crate) created_at: Option<String>,
    #[allow(dead_code)]
    pub(crate) updated_at: Option<String>,
    #[allow(dead_code)]
    pub(crate) setup_completed_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExecRow {
    pub(crate) id: Uuid,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) run_reason: Option<RunReason>,
    pub(crate) status: Option<ExecStatus>,
    pub(crate) created_at: Option<Timestamp>,
    pub(crate) dropped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunReason {
    CodingAgent,
    DevServer,
    Other,
}

impl RunReason {
    pub(crate) fn parse(s: &str) -> Self {
        match s {
            "coding_agent" => Self::CodingAgent,
            "dev_server" => Self::DevServer,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecStatus {
    Running,
    Completed,
    Failed,
    Unknown,
}

impl ExecStatus {
    pub(crate) fn parse(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "completed" | "done" | "success" => Self::Completed,
            "failed" | "error" => Self::Failed,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}
