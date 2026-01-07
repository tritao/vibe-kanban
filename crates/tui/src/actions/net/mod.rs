use crate::{events::NetEvent, state::AppState};

mod diff;
mod exec;
mod git;
mod logs;
mod projects;
mod tasks;
mod ui;

pub(super) fn reduce_net_event(app: &mut AppState, event: NetEvent) -> bool {
    match event {
        NetEvent::InfoLoaded { ok, summary } => projects::info_loaded(app, ok, summary),
        NetEvent::ExecutorProfilesLoaded {
            available,
            selected,
            profiles_executors,
        } => ui::executor_profiles_loaded(app, available, selected, profiles_executors),
        NetEvent::ProjectCreated { project_id } => projects::project_created(app, project_id),
        NetEvent::ProjectRepoAdded { project_id } => projects::project_repo_added(app, project_id),
        NetEvent::ProjectMatchResult { project_id } => {
            projects::project_match_result(app, project_id)
        }
        NetEvent::RepoBranchesLoaded { repo_id, branches } => {
            ui::repo_branches_loaded(app, repo_id, branches)
        }
        NetEvent::RepoBranchesFailed { repo_id, message } => {
            ui::repo_branches_failed(app, repo_id, message)
        }
        NetEvent::ProjectsStreamStatus(status) => projects::projects_stream_status(app, status),
        NetEvent::ProjectsPatch(patch) => projects::projects_patch(app, patch),
        NetEvent::TasksStreamStatus(status) => tasks::tasks_stream_status(app, status),
        NetEvent::TasksReset => tasks::tasks_reset(app),
        NetEvent::TasksPatch(patch) => tasks::tasks_patch(app, patch),
        NetEvent::AttemptsLoaded { task_id, attempts } => {
            tasks::attempts_loaded(app, task_id, attempts)
        }
        NetEvent::ExecStreamStatus(status) => exec::exec_stream_status(app, status),
        NetEvent::ExecReset => exec::exec_reset(app),
        NetEvent::ExecPatch(patch) => exec::exec_patch(app, patch),
        NetEvent::DiffStreamStatus(status) => diff::diff_stream_status(app, status),
        NetEvent::DiffReset => diff::diff_reset(app),
        NetEvent::DiffPatch(patch) => diff::diff_patch(app, patch),
        NetEvent::DiffReconnect => diff::diff_reconnect(app),
        NetEvent::DiffPreviewReady {
            generation,
            cache_key,
            cache_hash,
            width,
            lines,
        } => diff::diff_preview_ready(app, generation, cache_key, cache_hash, width, lines),
        NetEvent::LogPrewarmReady {
            exec_id,
            width,
            generation,
            cache,
        } => logs::log_prewarm_ready(app, exec_id, width, generation, cache),
        NetEvent::GitOpFinished {
            repo_id,
            kind,
            ok,
            message,
        } => git::git_op_finished(app, repo_id, kind, ok, message),
        NetEvent::LogStreamStatus(status) => logs::log_stream_status(app, status),
        NetEvent::LogReset(exec_id) => logs::log_reset(app, exec_id),
        NetEvent::LogPatch { exec_id, patch } => logs::log_patch(app, exec_id, patch),
        NetEvent::BranchStatusLoaded {
            attempt_id,
            statuses,
        } => diff::branch_status_loaded(app, attempt_id, statuses),
        NetEvent::StackStatusLoaded {
            repo_id,
            status,
            generation,
        } => git::stack_status_loaded(app, repo_id, status, generation),
        NetEvent::CommitListLoaded {
            repo_id,
            commits,
            append,
            has_more,
        } => git::commit_list_loaded(app, repo_id, commits, append, has_more),
        NetEvent::CommitPreviewLoaded {
            repo_id,
            text,
            generation,
        } => git::commit_preview_loaded(app, repo_id, text, generation),
        NetEvent::CommitPreviewFailed {
            repo_id,
            message,
            generation,
        } => git::commit_preview_failed(app, repo_id, message, generation),
        NetEvent::CommitListFailed { repo_id } => git::commit_list_failed(app, repo_id),
        NetEvent::TaskCreated { task_id, status } => tasks::task_created(app, task_id, status),
        NetEvent::Notice(msg) => ui::notice(app, msg),
        NetEvent::Error(msg) => ui::error(app, msg),
        NetEvent::ErrorKey { key, message } => ui::error_key(app, key, message),
    }
}
