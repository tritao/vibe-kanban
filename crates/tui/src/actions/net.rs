use std::time::Duration;

use crate::commands::{
    clear_pending_branch_status_refresh, finish_git_op, on_exec_store_updated_for_branch_refresh,
    request_diff_reconnect,
};
use crate::diff::{diff_rows_with_all, DIFF_ALL_KEY};
use crate::diff_preview::{
    diff_patch_touches_key, schedule_diff_preview_refresh,
};
use crate::events::{NetEvent, StreamStatus};
use crate::logs::{enqueue_log_patch, reset_logs};
use crate::logs::maybe_attach_pending_user_log;
use crate::state::AppState;
use crate::ui::sync_selected_repo_from_diff_selection;

use super::selection as sel;

pub(super) fn reduce_net_event(app: &mut AppState, event: NetEvent) -> bool {
    match event {
        NetEvent::InfoLoaded { ok, summary } => {
            app.info_ok = ok;
            app.info_summary = summary;
            true
        }
        NetEvent::ExecutorProfilesLoaded { available, selected } => {
            app.ui.available_executors = available;
            app.ui.selected_executor_profile = selected;
            true
        }
        NetEvent::ProjectsStreamStatus(status) => {
            app.board.projects_status = status;
            true
        }
        NetEvent::ProjectsPatch(patch) => {
            if let Err(e) = json_patch::patch(&mut app.board.projects_store, &patch) {
                app.ui.last_error = Some(format!("failed to apply projects patch: {e}"));
                app.board.projects_status = StreamStatus::Error;
                return true;
            }

            sel::reconcile_projects_selection(app);
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

            let rows = diff_rows_with_all(&app.diff.diff_store);
            if rows.is_empty() {
                return true;
            }
            let sel = app.diff.selected_diff_index.min(rows.len().saturating_sub(1));
            let sel_key = rows.get(sel).map(|r| r.key.as_str()).unwrap_or(DIFF_ALL_KEY);

            let should_refresh = if sel_key == DIFF_ALL_KEY {
                true
            } else {
                diff_patch_touches_key(&patch, sel_key)
            };
            if should_refresh {
                app.diff.diff_preview_cache_key = None;
                app.diff.diff_preview_cache_hash = 0;
                schedule_diff_preview_refresh(app, Duration::from_millis(0));
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
            true
        }
        NetEvent::GitOpFinished {
            repo_id,
            kind,
            ok,
            message,
        } => {
            finish_git_op(app, repo_id, kind, ok, message);
            true
        }
        NetEvent::LogStreamStatus(status) => {
            app.exec.log_status = status;
            true
        }
        NetEvent::LogReset(exec_id) => {
            reset_logs(app, exec_id);
            true
        }
        NetEvent::LogPatch { exec_id, patch } => {
            enqueue_log_patch(app, exec_id, patch);
            true
        }
        NetEvent::BranchStatusLoaded(statuses) => {
            app.diff.repo_statuses = statuses;
            sync_selected_repo_from_diff_selection(app);
            true
        }
        NetEvent::TaskCreated { task_id, status } => {
            // Place the new task in the expected column immediately, then select it when it appears.
            sel::note_task_created(app, task_id, status);
            true
        }
        NetEvent::Notice(msg) => {
            app.ui.last_notice = Some(msg);
            true
        }
        NetEvent::Error(msg) => {
            app.ui.last_error = Some(msg);
            true
        }
    }
}
