use std::hash::Hash;

use ratatui::text::Line;
use uuid::Uuid;

use crate::{
    logs::PreparedLogCache,
    state::{AttemptRow, ExecutorProfileSelection, GitBranchItem, RepoBranchStatus, TaskStatus},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamStatus {
    Connecting,
    Connected,
    Completed,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GitOpKind {
    Status,
    Merge,
    Rebase,
    CreatePr,
    Abort,
    Push,
    ForcePush,
    AttachPr,
    PrComments,
}

impl GitOpKind {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            GitOpKind::Status => "Status",
            GitOpKind::Merge => "Merge",
            GitOpKind::Rebase => "Rebase",
            GitOpKind::CreatePr => "Create PR",
            GitOpKind::Abort => "Abort",
            GitOpKind::Push => "Push",
            GitOpKind::ForcePush => "Force push",
            GitOpKind::AttachPr => "Attach PR",
            GitOpKind::PrComments => "PR comments",
        }
    }
}

#[derive(Debug)]
pub(crate) enum UiEvent {
    Crossterm(crossterm::event::Event),
    Tick,
}

#[derive(Debug)]
pub(crate) enum NetEvent {
    InfoLoaded {
        ok: bool,
        summary: String,
    },
    ExecutorProfilesLoaded {
        available: Vec<String>,
        selected: Option<ExecutorProfileSelection>,
        profiles_executors: serde_json::Value,
    },
    ProjectCreated {
        project_id: Uuid,
    },
    ProjectRepoAdded {
        project_id: Uuid,
    },
    ProjectMatchResult {
        project_id: Option<Uuid>,
    },
    RepoBranchesLoaded {
        repo_id: Uuid,
        branches: Vec<GitBranchItem>,
    },
    RepoBranchesFailed {
        repo_id: Uuid,
        message: String,
    },
    ProjectsStreamStatus(StreamStatus),
    ProjectsPatch(json_patch::Patch),
    TasksStreamStatus(StreamStatus),
    TasksReset,
    TasksPatch(json_patch::Patch),
    AttemptsLoaded {
        task_id: Uuid,
        attempts: Vec<AttemptRow>,
    },
    ExecStreamStatus(StreamStatus),
    ExecReset,
    ExecPatch(json_patch::Patch),
    DiffStreamStatus(StreamStatus),
    DiffReset,
    DiffPatch(json_patch::Patch),
    DiffReconnect,
    DiffPreviewReady {
        generation: u64,
        cache_key: Option<String>,
        cache_hash: u64,
        width: u16,
        lines: Vec<Line<'static>>,
    },
    LogPrewarmReady {
        exec_id: Uuid,
        width: u16,
        generation: u64,
        cache: PreparedLogCache,
    },
    GitOpFinished {
        repo_id: Option<Uuid>,
        kind: GitOpKind,
        ok: bool,
        message: String,
    },
    LogStreamStatus(StreamStatus),
    LogReset(Option<Uuid>),
    LogPatch {
        exec_id: Uuid,
        patch: json_patch::Patch,
    },
    BranchStatusLoaded {
        attempt_id: Uuid,
        statuses: Vec<RepoBranchStatus>,
    },
    StackStatusLoaded {
        repo_id: Uuid,
        status: crate::state::StackStatusResponse,
    },
    CommitListLoaded {
        repo_id: Uuid,
        commits: Vec<crate::state::CommitEntry>,
        append: bool,
        has_more: bool,
    },
    CommitListFailed {
        repo_id: Uuid,
    },
    CommitPreviewLoaded {
        repo_id: Uuid,
        lines: Vec<Line<'static>>,
    },
    CommitPreviewFailed {
        repo_id: Uuid,
        message: String,
    },
    TaskCreated {
        task_id: Uuid,
        status: TaskStatus,
    },
    Notice(String),
    Error(String),
}
