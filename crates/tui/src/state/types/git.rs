use serde::Deserialize;
use uuid::Uuid;

use super::ConflictOp;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitBranchItem {
    pub(crate) name: String,
    pub(crate) is_current: bool,
    pub(crate) is_remote: bool,
    #[allow(dead_code)]
    pub(crate) last_commit_date: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MergeStatus {
    Open,
    Merged,
    Closed,
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PullRequestInfo {
    pub(crate) number: i64,
    #[allow(dead_code)]
    pub(crate) url: String,
    pub(crate) status: MergeStatus,
    #[allow(dead_code)]
    pub(crate) merged_at: Option<String>,
    #[allow(dead_code)]
    pub(crate) merge_commit_sha: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PrMerge {
    pub(crate) pr_info: PullRequestInfo,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Merge {
    // We don't currently use any payload from "direct" merges; keep it as a unit struct so serde
    // can ignore any additional fields from the backend without storing untyped JSON.
    #[allow(dead_code)]
    Direct {},
    Pr(PrMerge),
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BranchStatus {
    pub(crate) commits_behind: Option<usize>,
    pub(crate) commits_ahead: Option<usize>,
    pub(crate) has_uncommitted_changes: Option<bool>,
    pub(crate) uncommitted_count: Option<usize>,
    pub(crate) untracked_count: Option<usize>,
    pub(crate) target_branch_name: String,
    pub(crate) remote_commits_behind: Option<usize>,
    pub(crate) remote_commits_ahead: Option<usize>,
    pub(crate) merges: Vec<Merge>,
    pub(crate) is_rebase_in_progress: bool,
    pub(crate) conflict_op: Option<ConflictOp>,
    pub(crate) conflicted_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RepoBranchStatus {
    pub(crate) repo_id: Uuid,
    pub(crate) repo_name: String,
    #[serde(default)]
    pub(crate) worktree_path: Option<String>,
    #[serde(flatten)]
    pub(crate) status: BranchStatus,
}
