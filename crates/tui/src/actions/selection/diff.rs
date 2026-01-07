use ratatui::text::Line;

use crate::{diff_preview::cancel_diff_preview_job, state::AppState};

pub(in crate::actions) fn reset_diff_stream_state(app: &mut AppState) {
    app.diff.diff_store = serde_json::json!({ "entries": {} });
    app.diff.selected_diff_index = 0;
    app.diff.diff_scroll_offset = 0;
    app.diff.invalidate_diff_preview_cache();
    app.diff.diff_preview_cache_width = 0;
    app.diff.diff_preview_lines = vec![Line::from("No diffs")];
    app.diff.diff_preview_pending = false;
    app.diff.diff_preview_next_refresh_at = None;
    cancel_diff_preview_job(app);

    app.diff.list_mode = crate::state::DiffListMode::Files;
    app.diff.selected_commit_index = 0;
    app.diff.commit_preview_text = None;
    app.diff.commit_preview_lines = vec![Line::from("No commit selected")];
    app.diff.commit_preview_render_width = 0;
    app.diff.commit_preview_loading.stop();
}
