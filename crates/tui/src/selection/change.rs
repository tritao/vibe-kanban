use std::time::Duration;

use uuid::Uuid;

use crate::{
    diff_preview::schedule_diff_preview_refresh,
    net,
    prefs::save_prefs,
    state::{AppState, DiffListMode},
};

pub(crate) fn reset_diff_stream_state(app: &mut AppState) {
    use ratatui::text::Line;

    app.diff.diff_store = serde_json::json!({ "entries": {} });
    app.diff.selected_diff_index = 0;
    app.diff.diff_scroll_offset = 0;
    app.diff.invalidate_diff_preview_cache();
    app.diff.diff_preview_cache_width = 0;
    app.diff.diff_preview_lines = vec![Line::from("No diffs")];
    app.diff.diff_preview_pending = false;
    app.diff.diff_preview_next_refresh_at = None;
    crate::diff_preview::cancel_diff_preview_job(app);

    app.diff.list_mode = crate::state::DiffListMode::Files;
    app.diff.selected_commit_index = 0;
    app.diff.commit_preview_text = None;
    app.diff.commit_preview_lines = vec![Line::from("No commit selected")];
    app.diff.commit_preview_render_width = 0;
    app.diff.commit_preview_loading.stop();
}

pub(crate) fn set_selected_repo_index(app: &mut AppState, idx: usize) {
    if app.diff.repo_statuses.is_empty() {
        app.diff.selected_repo_index = 0;
        return;
    }
    let idx = idx.min(app.diff.repo_statuses.len().saturating_sub(1));
    if app.diff.selected_repo_index == idx {
        return;
    }
    app.diff.selected_repo_index = idx;
    on_repo_selected(app);
}

pub(crate) fn on_repo_selected(app: &mut AppState) {
    crate::commands::request_stack_status_refresh(app);
    if app.diff.list_mode == DiffListMode::Commits {
        crate::commands::request_commit_list_refresh(app);
    }
    schedule_diff_preview_refresh(app, crate::ui::constants::DIFF_PREVIEW_REFRESH_DELAY);
}

pub(crate) fn on_diff_file_selected(app: &mut AppState) {
    app.diff.diff_scroll_offset = 0;
    crate::ui::sync_selected_repo_from_diff_selection(app);
    schedule_diff_preview_refresh(app, crate::ui::constants::DIFF_PREVIEW_REFRESH_DELAY);
}

pub(crate) fn on_commit_selected(app: &mut AppState) {
    app.diff.diff_scroll_offset = 0;
    crate::commands::request_commit_preview_refresh(app);
}

pub(crate) fn select_project(app: &mut AppState, project_id: Option<Uuid>) {
    if app.board.selected_project_id == project_id {
        return;
    }
    app.board.selected_project_id = project_id;
    let _ = app.project_sel_tx.send(project_id);
    app.prefs.selected_project_id = project_id;
    save_prefs(&app.prefs);
    select_task(app, None);
}

pub(crate) fn select_task(app: &mut AppState, task_id: Option<Uuid>) {
    if app.board.selected_task_id == task_id {
        return;
    }

    app.board.selected_task_id = task_id;

    app.board.attempts.clear();
    app.board.selected_attempt_index = 0;
    select_attempt(app, None);

    if let Some(task_id) = task_id {
        let base_url = app.backend_url.clone();
        let net_tx = app.net_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            net::load_attempts_task(base_url, task_id, net_tx).await;
        });
    }
}

pub(crate) fn select_attempt(app: &mut AppState, attempt_id: Option<Uuid>) {
    if app.board.selected_attempt_id == attempt_id && attempt_id.is_some() {
        return;
    }

    app.board.selected_attempt_id = attempt_id;
    if let Some(attempt_id) = attempt_id {
        if let Some(idx) = app.board.attempts.iter().position(|a| a.id == attempt_id) {
            app.board.selected_attempt_index = idx;
        }
    }

    app.ui.clear_messages();

    app.exec.exec_store = serde_json::json!({ "execution_processes": {} });
    select_exec(app, None);
    crate::logs::reset_log_view(app, None);

    reset_diff_stream_state(app);

    app.diff.repo_statuses.clear();
    app.diff.branch_status_loaded_attempt_id = None;
    app.diff.branch_status_loaded_at = None;
    app.diff.selected_repo_index = 0;

    let _ = app.attempt_sel_tx.send(attempt_id);
    crate::commands::schedule_branch_status_refresh_debounced(app, Duration::from_millis(250));
}

pub(crate) fn select_exec(app: &mut AppState, exec_id: Option<Uuid>) {
    if app.exec.selected_exec_id == exec_id {
        return;
    }

    app.exec.selected_exec_id = exec_id;
    app.ui.clear_messages();
    if let Some(exec_id) = exec_id {
        app.exec
            .log_buffers
            .entry(exec_id)
            .or_default()
            .ensure_init();
        app.exec.log_view_dirty = true;
    }
    app.exec.log_selected = None;
    let _ = app.exec_sel_tx.send(exec_id);
}
