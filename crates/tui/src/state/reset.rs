use ratatui::text::Line;

use crate::state::{AppState, DiffListMode};

pub(crate) fn reset_diff_stream_state(app: &mut AppState) {
    app.diff.diff_store = crate::store::roots::DiffRoot::empty();
    app.diff.selected_diff_index = 0;
    app.diff.diff_scroll.offset = 0;
    app.diff.invalidate_diff_preview_cache();
    app.diff.diff_preview_cache_width = 0;
    app.diff.diff_preview_lines = vec![Line::from(crate::ui::messages::placeholders::NO_DIFFS)];
    app.diff.diff_preview_pending = false;
    app.diff.diff_preview_next_refresh_at = None;
    crate::diff_preview::cancel_diff_preview_job(app);

    app.diff.list_mode = DiffListMode::Files;
    app.diff.selected_commit_index = 0;
    app.diff.commit_preview_text = None;
    app.diff.commit_preview_lines = vec![Line::from(
        crate::ui::messages::placeholders::NO_COMMIT_SELECTED,
    )];
    app.diff.commit_preview_render_width = 0;
    app.diff.commit_preview_loading.stop();
}

pub(crate) fn reset_exec_stream_state(app: &mut AppState) {
    app.exec.exec_store = crate::store::roots::ExecRoot::empty();
    app.exec.selected_exec_id = None;
    app.exec.log_selected = None;
    let _ = app.exec_sel_tx.send(None);
    crate::logs::reset_log_view(app, None);
}

pub(crate) fn reset_attempt_scoped_state(app: &mut AppState) {
    app.ui.clear_messages();

    reset_exec_stream_state(app);
    reset_diff_stream_state(app);

    app.diff.repo_statuses.clear();
    app.diff.branch_status_loaded_attempt_id = None;
    app.diff.branch_status_loaded_at = None;
    app.diff.branch_status_auto_next_at = None;
    app.diff.selected_repo_index = 0;
}
