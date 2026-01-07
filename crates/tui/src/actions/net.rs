use std::time::{Duration, Instant};

use super::selection as sel;
use crate::{
    commands::{
        clear_pending_branch_status_refresh, finish_git_op,
        on_exec_store_updated_for_branch_refresh, request_diff_reconnect,
    },
    diff::{DIFF_ALL_KEY, diff_rows_with_all_filtered},
    diff_preview::{
        diff_patch_touches_key, schedule_diff_preview_refresh,
        schedule_diff_preview_refresh_debounced,
    },
    events::{NetEvent, StreamStatus},
    logs::{enqueue_log_patch, maybe_attach_pending_user_log, reset_logs},
    net::ops::find_project_for_repo_path_http,
    state::{AppState, ProjectSetupState},
    ui::sync_selected_repo_from_diff_selection,
};

pub(super) fn reduce_net_event(app: &mut AppState, event: NetEvent) -> bool {
    match event {
        NetEvent::InfoLoaded { ok, summary } => {
            app.info_ok = ok;
            app.info_summary = summary;
            true
        }
        NetEvent::ExecutorProfilesLoaded {
            available,
            selected,
            profiles_executors,
        } => {
            app.ui.available_executors = available;
            app.ui.selected_executor_profile = selected;
            app.ui.executor_profiles = profiles_executors;
            true
        }
        NetEvent::ProjectCreated { project_id } => {
            app.ui.project_setup = None;
            app.ui.project_setup_dismissed = true;
            sel::select_project(app, Some(project_id));
            true
        }
        NetEvent::ProjectRepoAdded { project_id } => {
            app.ui.project_setup = None;
            app.ui.project_setup_dismissed = true;
            sel::select_project(app, Some(project_id));
            true
        }
        NetEvent::ProjectMatchResult { project_id } => {
            if app.ui.project_setup_dismissed {
                return false;
            }
            if let Some(project_id) = project_id {
                app.ui.project_setup = None;
                sel::select_project(app, Some(project_id));
                return true;
            }

            if app.ui.project_setup.is_none() {
                app.ui.project_setup = Some(ProjectSetupState {
                    repo_path: app.ui.launch_repo_path.clone(),
                    suggested_project_name: app.ui.launch_suggested_project_name.clone(),
                    has_projects: true,
                    busy: false,
                });
                return true;
            }
            false
        }
        NetEvent::RepoBranchesLoaded { repo_id, branches } => {
            if let Some(state) = app.ui.branch_picker.as_mut() {
                if state.repo_id == repo_id {
                    state.branches = branches;
                    state.busy = false;
                    state.error = None;
                    state.selected_index = state
                        .selected_index
                        .min(state.branches.len().saturating_sub(1));
                    return true;
                }
            }
            false
        }
        NetEvent::RepoBranchesFailed { repo_id, message } => {
            if let Some(state) = app.ui.branch_picker.as_mut() {
                if state.repo_id == repo_id {
                    state.busy = false;
                    state.error = Some(message);
                    return true;
                }
            }
            false
        }
        NetEvent::ProjectsStreamStatus(status) => {
            app.board.projects_status = status;
            true
        }
        NetEvent::ProjectsPatch(patch) => {
            fn ensure_projects_root(v: &mut serde_json::Value) {
                if !v.is_object() {
                    *v = serde_json::json!({});
                }
                let obj = v.as_object_mut().expect("object");
                if !obj.contains_key("projects")
                    || !obj.get("projects").is_some_and(|p| p.is_object())
                {
                    obj.insert("projects".to_string(), serde_json::json!({}));
                }
            }

            fn patch_with_projects_upserts(patch: &json_patch::Patch) -> json_patch::Patch {
                use json_patch::{AddOperation, PatchOperation, ReplaceOperation};
                let mut out: Vec<PatchOperation> = Vec::with_capacity(patch.0.len());
                for op in patch.iter() {
                    match op {
                        PatchOperation::Replace(ReplaceOperation { path, value }) => {
                            let p = path.to_string();
                            if p.starts_with("/projects/") {
                                out.push(PatchOperation::Add(AddOperation {
                                    path: path.clone(),
                                    value: value.clone(),
                                }));
                            } else {
                                out.push(PatchOperation::Replace(ReplaceOperation {
                                    path: path.clone(),
                                    value: value.clone(),
                                }));
                            }
                        }
                        other => out.push(other.clone()),
                    }
                }
                json_patch::Patch(out)
            }

            ensure_projects_root(&mut app.board.projects_store);
            if let Err(e) = json_patch::patch(&mut app.board.projects_store, &patch) {
                // Fallback: treat project replaces as upserts to handle cases where we missed an
                // insert patch but still receive a replace for that ID.
                let patched = patch_with_projects_upserts(&patch);
                if let Err(e2) = json_patch::patch(&mut app.board.projects_store, &patched) {
                    app.ui.last_error = Some(format!(
                        "failed to apply projects patch: {e} (fallback also failed: {e2})"
                    ));
                    app.board.projects_status = StreamStatus::Error;
                    return true;
                }
            }
            app.board.projects_loaded_once = true;

            sel::reconcile_projects_selection(app);
            let projects_empty =
                crate::selection::lists_filters::projects_list(&app.board.projects_store)
                    .is_empty();

            if app.ui.launch_dir_explicit
                && !app.ui.launch_match_done
                && app.ui.launch_repo_path.is_some()
                && !app.ui.project_setup_dismissed
            {
                app.ui.launch_match_done = true;
                let base_url = app.backend_url.clone();
                let net_tx = app.net_tx.clone();
                let repo_path = app.ui.launch_repo_path.clone().unwrap_or_default();
                tokio::spawn(async move {
                    let matched = find_project_for_repo_path_http(&base_url, &repo_path).await;
                    match matched {
                        Ok(project_id) => {
                            let _ = net_tx
                                .send(NetEvent::ProjectMatchResult { project_id })
                                .await;
                        }
                        Err(e) => {
                            let _ = net_tx
                                .send(NetEvent::Error(format!("project match failed: {e}")))
                                .await;
                            let _ = net_tx
                                .send(NetEvent::ProjectMatchResult { project_id: None })
                                .await;
                        }
                    }
                });
            }

            if app.board.projects_loaded_once
                && projects_empty
                && app.ui.project_setup.is_none()
                && !app.ui.project_setup_dismissed
            {
                app.ui.project_setup = Some(ProjectSetupState {
                    repo_path: app.ui.launch_repo_path.clone(),
                    suggested_project_name: app.ui.launch_suggested_project_name.clone(),
                    has_projects: false,
                    busy: false,
                });
                return true;
            }
            true
        }
        NetEvent::TasksStreamStatus(status) => {
            app.board.tasks_status = status;
            true
        }
        NetEvent::TasksReset => {
            app.board.tasks_store = serde_json::json!({ "tasks": {} });
            sel::select_task(app, None);
            app.board.pending_select_task_id = None;
            true
        }
        NetEvent::TasksPatch(patch) => {
            if let Err(e) = json_patch::patch(&mut app.board.tasks_store, &patch) {
                app.ui.last_error = Some(format!("failed to apply tasks patch: {e}"));
                app.board.tasks_status = StreamStatus::Error;
                return true;
            }
            sel::reconcile_tasks_selection(app);
            true
        }
        NetEvent::AttemptsLoaded { task_id, attempts } => {
            if app.board.selected_task_id != Some(task_id) {
                return false;
            }

            sel::set_attempts(app, attempts);
            true
        }
        NetEvent::ExecStreamStatus(status) => {
            app.exec.exec_status = status;
            true
        }
        NetEvent::ExecReset => {
            app.exec.exec_store = serde_json::json!({ "execution_processes": {} });
            sel::select_exec(app, None);
            clear_pending_branch_status_refresh(app);
            true
        }
        NetEvent::ExecPatch(patch) => {
            if let Err(e) = json_patch::patch(&mut app.exec.exec_store, &patch) {
                app.ui.last_error = Some(format!("failed to apply exec patch: {e}"));
                app.exec.exec_status = StreamStatus::Error;
                return true;
            }
            sel::ensure_exec_selection(app);
            maybe_attach_pending_user_log(app);
            on_exec_store_updated_for_branch_refresh(app);
            true
        }
        NetEvent::DiffStreamStatus(status) => {
            app.diff.diff_status = status;
            true
        }
        NetEvent::DiffReset => {
            sel::reset_diff_stream_state(app);
            true
        }
        NetEvent::DiffPatch(patch) => {
            if let Err(e) = json_patch::patch(&mut app.diff.diff_store, &patch) {
                app.ui.last_error = Some(format!("failed to apply diff patch: {e}"));
                app.diff.diff_status = StreamStatus::Error;
                return true;
            }

            let touches_entries = patch.iter().any(|op| {
                let path = op.path().to_string();
                path == "/entries" || path.starts_with("/entries/")
            });
            if !touches_entries {
                return true;
            }

            let rows =
                diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
            if rows.is_empty() {
                return true;
            }
            let sel = app
                .diff
                .selected_diff_index
                .min(rows.len().saturating_sub(1));
            let sel_key = rows
                .get(sel)
                .map(|r| r.key.as_str())
                .unwrap_or(DIFF_ALL_KEY);

            let should_refresh = if sel_key == DIFF_ALL_KEY {
                true
            } else {
                diff_patch_touches_key(&patch, sel_key)
            };
            if should_refresh {
                app.diff.diff_preview_cache_key = None;
                app.diff.diff_preview_cache_hash = 0;
                // The diff stream can send many patches during initial load (one per file).
                // Rebuilding the combined "__ALL__" preview on every patch is very expensive and
                // looks like the view is “growing” line-by-line. Debounce in ALL mode.
                if sel_key == DIFF_ALL_KEY {
                    schedule_diff_preview_refresh_debounced(app, Duration::from_millis(120));
                } else {
                    schedule_diff_preview_refresh(app, Duration::from_millis(0));
                }
            }
            true
        }
        NetEvent::DiffReconnect => {
            sel::reset_diff_stream_state(app);
            request_diff_reconnect(app);
            true
        }
        NetEvent::DiffPreviewReady {
            generation,
            cache_key,
            cache_hash,
            width,
            lines,
        } => {
            if generation != app.diff.diff_preview_gen {
                return false;
            }
            app.diff.diff_preview_cache_key = cache_key;
            app.diff.diff_preview_cache_hash = cache_hash;
            app.diff.diff_preview_cache_width = width;
            app.diff.diff_preview_lines = lines;
            app.diff.diff_preview_loading = false;
            app.diff.diff_preview_loading_started_at = None;
            app.diff.diff_preview_loading_placeholder_pending = false;
            true
        }
        NetEvent::LogPrewarmReady {
            exec_id,
            width,
            generation,
            cache,
        } => {
            if generation != app.exec.log_prewarm_gen {
                return false;
            }
            if width != app.exec.log_target_render_width {
                return false;
            }
            if let Some(buf) = app.exec.log_buffers.get_mut(&exec_id) {
                buf.install_cache(width, cache);
            }
            false
        }
        NetEvent::GitOpFinished {
            repo_id,
            kind,
            ok,
            message,
        } => {
            finish_git_op(app, repo_id, kind, ok, message);
            if ok && app.diff.list_mode == crate::state::DiffListMode::Commits {
                let selected_repo_id = app
                    .diff
                    .repo_statuses
                    .get(app.diff.selected_repo_index)
                    .map(|r| r.repo_id);
                if repo_id.is_none() || repo_id == selected_repo_id {
                    crate::commands::request_commit_list_refresh(app);
                }
            }
            true
        }
        NetEvent::LogStreamStatus(status) => {
            app.exec.log_status = status;
            if matches!(status, StreamStatus::Connected | StreamStatus::Completed) {
                if let Some(err) = app.ui.last_error.as_deref()
                    && err.starts_with("log stream connect:")
                {
                    app.ui.last_error = None;
                }
            }
            true
        }
        NetEvent::LogReset(exec_id) => {
            // Keep cached buffers unless we're explicitly resetting a specific exec buffer.
            match exec_id {
                Some(id) => reset_logs(app, Some(id)),
                None => crate::logs::reset_log_view(app, None),
            }
            true
        }
        NetEvent::LogPatch { exec_id, patch } => {
            enqueue_log_patch(app, app.board.selected_attempt_id, exec_id, patch);
            true
        }
        NetEvent::BranchStatusLoaded {
            attempt_id,
            statuses,
        } => {
            if app.board.selected_attempt_id != Some(attempt_id) {
                return true;
            }
            app.diff.repo_statuses = statuses;
            app.diff.branch_status_loaded_attempt_id = Some(attempt_id);
            app.diff.branch_status_loaded_at = Some(Instant::now());
            sync_selected_repo_from_diff_selection(app);
            crate::commands::request_stack_status_refresh(app);
            crate::commands::request_commit_list_refresh(app);
            true
        }
        NetEvent::StackStatusLoaded { repo_id, status } => {
            app.diff.stack_status_by_repo.insert(repo_id, status);
            true
        }
        NetEvent::CommitListLoaded {
            repo_id,
            commits,
            append,
            has_more,
        } => {
            crate::commands::apply_commit_list_page(app, repo_id, commits, append, has_more);
            true
        }
        NetEvent::CommitPreviewLoaded { repo_id, lines } => {
            // Only update the preview if we're still looking at this repo.
            let selected_repo_id = app
                .diff
                .repo_statuses
                .get(app.diff.selected_repo_index)
                .map(|r| r.repo_id);
            if selected_repo_id == Some(repo_id) {
                app.diff.commit_preview_lines = lines;
                app.diff.commit_preview_loading = false;
            }
            true
        }
        NetEvent::CommitListFailed { repo_id } => {
            app.diff.commits_loading_by_repo.insert(repo_id, false);
            true
        }
        NetEvent::TaskCreated { task_id, status } => {
            // Place the new task in the expected column immediately, then select it when it appears.
            sel::note_task_created(app, task_id, status);
            // The tasks stream patch can arrive before the create-task HTTP call returns.
            // If the task is already present in the store, reconcile now so the new task is
            // selected immediately; otherwise `pending_select_task_id` will be picked up on the
            // next tasks patch.
            sel::reconcile_tasks_selection(app);
            true
        }
        NetEvent::Notice(msg) => {
            app.ui.last_notice = Some(msg);
            true
        }
        NetEvent::Error(msg) => {
            app.ui.last_error = Some(msg);
            if let Some(state) = app.ui.project_setup.as_mut() {
                state.busy = false;
            }
            true
        }
    }
}
