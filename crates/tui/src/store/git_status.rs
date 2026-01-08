use uuid::Uuid;

use crate::state::{ConflictOp, Merge, MergeStatus, PullRequestInfo, RepoBranchStatus};

#[derive(Debug, Clone, Copy)]
pub(crate) enum GitRepoAction {
    Merge,
    Rebase,
    CreatePr,
    OpenPr,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum GitActionBlockSeverity {
    Ok,
    Warn,
}

#[derive(Debug, Clone)]
pub(crate) struct GitActionBlock {
    pub(crate) severity: GitActionBlockSeverity,
    pub(crate) message: String,
}

pub(crate) struct RepoStatuses<'a> {
    repos: &'a [RepoBranchStatus],
}

impl<'a> RepoStatuses<'a> {
    pub(crate) fn new(repos: &'a [RepoBranchStatus]) -> Self {
        Self { repos }
    }

    pub(crate) fn get_by_id(&self, repo_id: Uuid) -> Option<RepoStatusRef<'a>> {
        self.repos
            .iter()
            .find(|r| r.repo_id == repo_id)
            .map(RepoStatusRef::new)
    }

    #[allow(dead_code)]
    pub(crate) fn get_by_index(&self, idx: usize) -> Option<RepoStatusRef<'a>> {
        self.repos.get(idx).map(RepoStatusRef::new)
    }

    pub(crate) fn first_conflicts_index(&self) -> Option<usize> {
        self.repos
            .iter()
            .position(|r| RepoStatusRef::new(r).has_conflicts())
    }

    pub(crate) fn list_lines(&self, selected_idx: usize) -> Vec<String> {
        let mut lines = vec!["Repos:".to_string()];
        for (idx, repo) in self.repos.iter().enumerate() {
            let marker = if idx == selected_idx { "*" } else { " " };
            lines.push(format!("  {marker} {}. {}", idx + 1, repo.repo_name));
        }
        lines
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RepoStatusRef<'a> {
    repo: &'a RepoBranchStatus,
}

impl<'a> RepoStatusRef<'a> {
    pub(crate) fn new(repo: &'a RepoBranchStatus) -> Self {
        Self { repo }
    }

    #[allow(dead_code)]
    pub(crate) fn repo_id(self) -> Uuid {
        self.repo.repo_id
    }

    pub(crate) fn repo_name(self) -> &'a str {
        &self.repo.repo_name
    }

    #[allow(dead_code)]
    pub(crate) fn worktree_path(self) -> Option<&'a str> {
        self.repo.worktree_path.as_deref()
    }

    pub(crate) fn commits_ahead(self) -> usize {
        self.repo.status.commits_ahead.unwrap_or(0)
    }

    pub(crate) fn commits_behind(self) -> usize {
        self.repo.status.commits_behind.unwrap_or(0)
    }

    pub(crate) fn remote_commits_ahead(self) -> usize {
        self.repo.status.remote_commits_ahead.unwrap_or(0)
    }

    pub(crate) fn remote_commits_behind(self) -> usize {
        self.repo.status.remote_commits_behind.unwrap_or(0)
    }

    pub(crate) fn target_branch_name(self) -> &'a str {
        &self.repo.status.target_branch_name
    }

    pub(crate) fn uncommitted_count(self) -> usize {
        self.repo.status.uncommitted_count.unwrap_or(0)
    }

    pub(crate) fn untracked_count(self) -> usize {
        self.repo.status.untracked_count.unwrap_or(0)
    }

    pub(crate) fn is_rebase_in_progress(self) -> bool {
        self.repo.status.is_rebase_in_progress
    }

    pub(crate) fn conflict_op(self) -> Option<ConflictOp> {
        self.repo.status.conflict_op
    }

    pub(crate) fn conflicted_files(self) -> &'a [String] {
        &self.repo.status.conflicted_files
    }

    pub(crate) fn conflicts_count(self) -> usize {
        self.repo.status.conflicted_files.len()
    }

    pub(crate) fn has_conflicts(self) -> bool {
        self.is_rebase_in_progress() || self.conflicts_count() > 0
    }

    pub(crate) fn is_dirty(self) -> bool {
        self.repo.status.has_uncommitted_changes.unwrap_or(false)
            || self.uncommitted_count() > 0
            || self.untracked_count() > 0
    }

    pub(crate) fn pr_info(self) -> Option<&'a PullRequestInfo> {
        self.repo.status.merges.iter().find_map(|m| match m {
            Merge::Pr(pr) => Some(&pr.pr_info),
            _ => None,
        })
    }

    pub(crate) fn pr_number(self) -> Option<i64> {
        self.pr_info().map(|p| p.number)
    }

    pub(crate) fn pr_url(self) -> Option<&'a str> {
        self.pr_info().map(|p| p.url.as_str())
    }

    pub(crate) fn pr_status(self) -> Option<MergeStatus> {
        self.pr_info().map(|p| p.status)
    }

    pub(crate) fn pr_badge(self) -> Option<(i64, MergeStatus)> {
        self.pr_number().zip(self.pr_status())
    }

    pub(crate) fn can_merge(self) -> bool {
        self.action_block(GitRepoAction::Merge).is_none()
    }

    pub(crate) fn can_rebase(self) -> bool {
        self.action_block(GitRepoAction::Rebase).is_none()
    }

    pub(crate) fn can_create_pr(self) -> bool {
        self.action_block(GitRepoAction::CreatePr).is_none()
    }

    pub(crate) fn can_open_pr(self) -> bool {
        self.action_block(GitRepoAction::OpenPr).is_none()
    }

    pub(crate) fn action_block(self, action: GitRepoAction) -> Option<GitActionBlock> {
        let conflicts = self.has_conflicts();
        let dirty = self.is_dirty();

        match action {
            GitRepoAction::Merge => {
                if conflicts {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Warn,
                        message: "Merge: conflicts in progress (resolve/abort first)".to_string(),
                    });
                }
                if dirty {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Warn,
                        message: "Merge: repo has uncommitted changes".to_string(),
                    });
                }
                if self.commits_ahead() == 0 {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Ok,
                        message: "Merge: nothing to merge (up to date)".to_string(),
                    });
                }
            }
            GitRepoAction::Rebase => {
                if conflicts {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Warn,
                        message: "Rebase: conflicts in progress (resolve/abort first)".to_string(),
                    });
                }
                if dirty {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Warn,
                        message: "Rebase: repo has uncommitted changes".to_string(),
                    });
                }
                if self.commits_behind() == 0 {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Ok,
                        message: "Rebase: already up to date".to_string(),
                    });
                }
            }
            GitRepoAction::CreatePr => {
                if conflicts {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Warn,
                        message: "PR: conflicts in progress (resolve/abort first)".to_string(),
                    });
                }
                if dirty {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Warn,
                        message: "PR: repo has uncommitted changes".to_string(),
                    });
                }
                if self.commits_ahead() == 0 {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Ok,
                        message: "PR: no changes to open (up to date)".to_string(),
                    });
                }
                if let Some(n) = self.pr_number() {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Ok,
                        message: format!("PR: already exists (PR#{n})"),
                    });
                }
            }
            GitRepoAction::OpenPr => {
                if self.pr_number().is_none() {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Warn,
                        message: "PR: none attached".to_string(),
                    });
                }
                if self.pr_url().is_none_or(|u| u.trim().is_empty()) {
                    return Some(GitActionBlock {
                        severity: GitActionBlockSeverity::Warn,
                        message: "PR: has no URL".to_string(),
                    });
                }
            }
        }

        None
    }
}
