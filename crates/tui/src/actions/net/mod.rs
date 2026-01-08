use crate::{events::NetEvent, state::AppState};

mod apply;
mod diff;
mod exec;
mod git;
mod logs;
mod projects;
mod tasks;
mod ui;

use apply::{NetApplyResult, apply_effects};

impl NetEvent {
    pub(crate) fn apply(self, app: &mut AppState) -> bool {
        let result: NetApplyResult = match self {
            NetEvent::InfoLoaded { ok, summary } => {
                NetApplyResult::changed(projects::info_loaded(app, ok, summary))
            }
            NetEvent::ExecutorProfilesLoaded {
                available,
                selected,
                profiles_executors,
            } => NetApplyResult::changed(ui::executor_profiles_loaded(
                app,
                available,
                selected,
                profiles_executors,
            )),
            NetEvent::ProjectCreated { project_id } => {
                NetApplyResult::changed(projects::project_created(app, project_id))
            }
            NetEvent::ProjectRepoAdded { project_id } => {
                NetApplyResult::changed(projects::project_repo_added(app, project_id))
            }
            NetEvent::ProjectMatchResult { project_id } => {
                NetApplyResult::changed(projects::project_match_result(app, project_id))
            }
            NetEvent::RepoBranchesLoaded { repo_id, branches } => {
                NetApplyResult::changed(ui::repo_branches_loaded(app, repo_id, branches))
            }
            NetEvent::RepoBranchesFailed { repo_id, message } => {
                NetApplyResult::changed(ui::repo_branches_failed(app, repo_id, message))
            }
            NetEvent::ProjectsStreamStatus(status) => {
                NetApplyResult::changed(projects::projects_stream_status(app, status))
            }
            NetEvent::ProjectsPatch(patch) => {
                NetApplyResult::changed(projects::projects_patch(app, patch))
            }
            NetEvent::TasksStreamStatus(status) => {
                NetApplyResult::changed(tasks::tasks_stream_status(app, status))
            }
            NetEvent::TasksReset => NetApplyResult::changed(tasks::tasks_reset(app)),
            NetEvent::TasksPatch(patch) => NetApplyResult::changed(tasks::tasks_patch(app, patch)),
            NetEvent::AttemptsLoaded { task_id, attempts } => {
                NetApplyResult::changed(tasks::attempts_loaded(app, task_id, attempts))
            }
            NetEvent::ExecStreamStatus(status) => {
                NetApplyResult::changed(exec::exec_stream_status(app, status))
            }
            NetEvent::ExecReset => NetApplyResult::changed(exec::exec_reset(app)),
            NetEvent::ExecPatch(patch) => NetApplyResult::changed(exec::exec_patch(app, patch)),
            NetEvent::DiffStreamStatus(status) => {
                NetApplyResult::changed(diff::diff_stream_status(app, status))
            }
            NetEvent::DiffReset => NetApplyResult::changed(diff::diff_reset(app)),
            NetEvent::DiffPatch(patch) => NetApplyResult::changed(diff::diff_patch(app, patch)),
            NetEvent::DiffReconnect => NetApplyResult::changed(diff::diff_reconnect(app)),
            NetEvent::DiffPreviewReady {
                generation,
                cache_key,
                cache_hash,
                width,
                lines,
            } => NetApplyResult::changed(diff::diff_preview_ready(
                app, generation, cache_key, cache_hash, width, lines,
            )),
            NetEvent::LogPrewarmReady {
                exec_id,
                width,
                generation,
                cache,
            } => NetApplyResult::changed(logs::log_prewarm_ready(
                app, exec_id, width, generation, cache,
            )),
            NetEvent::GitOpFinished {
                repo_id,
                kind,
                ok,
                message,
            } => NetApplyResult::changed(git::git_op_finished(app, repo_id, kind, ok, message)),
            NetEvent::LogStreamStatus(status) => {
                NetApplyResult::changed(logs::log_stream_status(app, status))
            }
            NetEvent::LogReset(exec_id) => NetApplyResult::changed(logs::log_reset(app, exec_id)),
            NetEvent::LogPatch { exec_id, patch } => {
                NetApplyResult::changed(logs::log_patch(app, exec_id, patch))
            }
            NetEvent::BranchStatusLoaded {
                attempt_id,
                statuses,
            } => diff::branch_status_loaded(app, attempt_id, statuses),
            NetEvent::StackStatusLoaded {
                repo_id,
                status,
                generation,
            } => {
                NetApplyResult::changed(git::stack_status_loaded(app, repo_id, status, generation))
            }
            NetEvent::CommitListLoaded {
                repo_id,
                commits,
                append,
                has_more,
            } => NetApplyResult::changed(git::commit_list_loaded(
                app, repo_id, commits, append, has_more,
            )),
            NetEvent::CommitPreviewLoaded {
                repo_id,
                text,
                generation,
            } => {
                NetApplyResult::changed(git::commit_preview_loaded(app, repo_id, text, generation))
            }
            NetEvent::CommitPreviewFailed {
                repo_id,
                message,
                generation,
            } => NetApplyResult::changed(git::commit_preview_failed(
                app, repo_id, message, generation,
            )),
            NetEvent::CommitListFailed { repo_id } => {
                NetApplyResult::changed(git::commit_list_failed(app, repo_id))
            }
            NetEvent::TaskCreated { task_id, status } => {
                NetApplyResult::changed(tasks::task_created(app, task_id, status))
            }
            NetEvent::Notice(msg) => NetApplyResult::changed(ui::notice(app, msg)),
            NetEvent::Error(msg) => NetApplyResult::changed(ui::error(app, msg)),
            NetEvent::ErrorKey { key, message } => {
                NetApplyResult::changed(ui::error_key(app, key, message))
            }
        };
        apply_effects(app, result.effects);
        result.changed
    }
}

pub(super) fn reduce_net_event(app: &mut AppState, event: NetEvent) -> bool {
    event.apply(app)
}
