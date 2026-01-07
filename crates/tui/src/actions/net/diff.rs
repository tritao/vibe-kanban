use std::time::Instant;

use crate::{
    actions::selection as sel,
    commands::request_diff_reconnect,
    diff::{DIFF_ALL_KEY, diff_rows_with_all_filtered},
    diff_preview::{
        diff_patch_touches_key, schedule_diff_preview_refresh,
        schedule_diff_preview_refresh_debounced,
    },
    events::StreamStatus,
    state::{AppState, RepoBranchStatus},
    ui::sync_selected_repo_from_diff_selection,
};

pub(super) fn diff_stream_status(app: &mut AppState, status: StreamStatus) -> bool {
    app.diff.diff_status = status;
    true
}

pub(super) fn diff_reset(app: &mut AppState) -> bool {
    sel::reset_diff_stream_state(app);
    true
}

pub(super) fn diff_patch(app: &mut AppState, patch: json_patch::Patch) -> bool {
    if let Err(e) = json_patch::patch(&mut app.diff.diff_store, &patch) {
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

    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
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
        app.diff.invalidate_diff_preview_cache();
        // The diff stream can send many patches during initial load (one per file).
        // Rebuilding the combined "__ALL__" preview on every patch is very expensive and
        // looks like the view is “growing” line-by-line. Debounce in ALL mode.
        if sel_key == DIFF_ALL_KEY {
            schedule_diff_preview_refresh_debounced(
                app,
                crate::ui::constants::DIFF_ALL_DEBOUNCE_DELAY,
            );
        } else {
            schedule_diff_preview_refresh(app, crate::ui::constants::DIFF_PREVIEW_REFRESH_DELAY);
        }
    }
    true
}

pub(super) fn diff_reconnect(app: &mut AppState) -> bool {
    sel::reset_diff_stream_state(app);
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
) -> bool {
    if generation != app.diff.diff_preview_gen {
        return false;
    }
    app.diff.diff_preview_cache_key = cache_key;
    app.diff.diff_preview_cache_hash = cache_hash;
    app.diff.diff_preview_cache_width = width;
    app.diff.diff_preview_lines = lines;
    app.diff.diff_preview_loading.stop();
    true
}

pub(super) fn branch_status_loaded(
    app: &mut AppState,
    attempt_id: uuid::Uuid,
    statuses: Vec<RepoBranchStatus>,
) -> bool {
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
