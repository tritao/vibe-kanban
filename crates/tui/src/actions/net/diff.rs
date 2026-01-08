use std::time::Instant;

use super::apply::{NetApplyResult, NetEffects};
use crate::{
    commands::request_diff_reconnect,
    diff_preview::on_diff_entries_patched,
    events::StreamStatus,
    state::{AppState, RepoBranchStatus},
};

pub(super) fn diff_stream_status(app: &mut AppState, status: StreamStatus) -> bool {
    super::stream::apply_status(&mut app.diff.diff_status, status)
}

pub(super) fn diff_reset(app: &mut AppState) -> bool {
    crate::state::reset::reset_diff_stream_state(app);
    true
}

pub(super) fn diff_patch(app: &mut AppState, patch: json_patch::Patch) -> bool {
    if let Err(e) = app.diff.diff_store.apply_patch(&patch) {
        app.ui.set_error(format!("failed to apply diff patch: {e}"));
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
    on_diff_entries_patched(app, &patch);
    true
}

pub(super) fn diff_reconnect(app: &mut AppState) -> bool {
    crate::state::reset::reset_diff_stream_state(app);
    request_diff_reconnect(app);
    true
}

pub(super) fn diff_preview_ready(
    app: &mut AppState,
    generation: u64,
    cache_key: Option<String>,
    cache_hash: u64,
    width: u16,
    lines: Vec<ratatui::text::Line<'static>>,
) -> super::apply::NetApplyResult {
    if !app.diff.diff_preview_gen.is_latest(generation) {
        return super::apply::NetApplyResult::changed(false);
    }
    app.diff.diff_preview_cache_key = cache_key;
    app.diff.diff_preview_cache_hash = cache_hash;
    app.diff.diff_preview_cache_width = width;
    app.diff.diff_preview_lines = lines;
    app.diff.diff_preview_loading.stop();
    super::apply::NetApplyResult::changed(true)
}

pub(super) fn branch_status_loaded(
    app: &mut AppState,
    attempt_id: uuid::Uuid,
    statuses: Vec<RepoBranchStatus>,
) -> NetApplyResult {
    if app.board.selected_attempt_id != Some(attempt_id) {
        return NetApplyResult::changed(true);
    }
    app.ui
        .clear_error_scope(crate::state::UiMessageKey::BranchStatus);
    app.diff.branch_status_loading_notice.stop();
    app.diff.repo_statuses = statuses;
    app.diff.branch_status_loaded_attempt_id = Some(attempt_id);
    app.diff.branch_status_loaded_at = Some(Instant::now());
    app.diff.branch_status_auto_next_at =
        Some(Instant::now() + crate::ui::constants::BRANCH_STATUS_AUTO_REFRESH_INTERVAL);
    NetApplyResult::changed(true).with_effects(NetEffects {
        sync_selected_repo: true,
        refresh_stack_status: true,
        refresh_commit_list: true,
    })
}
