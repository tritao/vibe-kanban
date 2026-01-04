use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::commands::update_git_activity_indicators;
use crate::diff_preview::{
    diff_preview_refresh_ready, request_diff_preview_async, schedule_diff_preview_refresh,
};
use crate::jobs::{job_running, reap_finished_jobs};
use crate::layout::{clamp_scroll_offsets, compute_main_layout};
use crate::logs::{flush_log_buffers, prewarm_log_cache_for_exec};
use crate::state::{AppState, JobKey};
use crate::selection::exec_list;

pub(super) fn reduce_tick(app: &mut AppState, now: Instant, term: Rect) -> bool {
    let mut dirty = false;

    if reap_finished_jobs(app) {
        dirty = true;
    }

    let layout = compute_main_layout(term, app.ui.focus);
    let inner_width = layout.exec_logs.width.saturating_sub(2);
    let target_width = inner_width;
    if app.exec.log_target_render_width != target_width {
        app.exec.log_target_render_width = target_width;
        app.exec.log_prewarm_cursor = 0;
    }
    if app.exec.log_render_width == 0 {
        app.exec.log_render_width = target_width;
    }

    // Keep the currently displayed width up-to-date (fast path).
    if flush_log_buffers(app, app.exec.log_render_width as usize) {
        dirty = true;
    }

    // If the pane width changed (e.g. diff focus zoom), pre-warm caches for the target width in
    // small chunks and only switch once the primary buffer is ready.
    if app.exec.log_render_width != app.exec.log_target_render_width
        && !app.exec.log_buffers.is_empty()
    {
        let mut execs = exec_list(&app.exec.exec_store);
        execs.sort_by_key(|e| e.created_at.clone().unwrap_or_default());
        let primary_opt = app
            .exec
            .selected_exec_id
            .or_else(|| execs.last().map(|e| e.id));

        // Prewarm one buffer per tick (round-robin over visible execs).
        let include: Vec<uuid::Uuid> = match app.exec.log_view_mode {
            crate::state::LogViewMode::Single => primary_opt.into_iter().collect(),
            crate::state::LogViewMode::Timeline => execs.iter().map(|e| e.id).collect(),
        };
        if !include.is_empty() {
            let idx = app.exec.log_prewarm_cursor % include.len();
            let id = include[idx];
            prewarm_log_cache_for_exec(app, id, app.exec.log_target_render_width as usize);
            app.exec.log_prewarm_cursor = app.exec.log_prewarm_cursor.wrapping_add(1);
        }

        // If the primary execution's cache is ready for the target width, swap.
        if let Some(primary_id) = primary_opt {
            if let Some(buf) = app.exec.log_buffers.get(&primary_id) {
                if buf.cache_ready(app.exec.log_target_render_width) {
                    app.exec.log_render_width = app.exec.log_target_render_width;
                    app.exec.log_view_dirty = true;
                    dirty = true;
                }
            }
        }
    }

    let diff_inner_width_u16 = layout.diff_preview.width.saturating_sub(2);
    let diff_inner_width = diff_inner_width_u16 as usize;
    let diff_width_changed = app.diff.diff_preview_cache_width != diff_inner_width_u16;
    let has_diffs = app
        .diff
        .diff_store
        .get("entries")
        .and_then(|v| v.as_object())
        .is_some_and(|o| !o.is_empty());
    if diff_width_changed {
        app.diff.diff_preview_cache_width = diff_inner_width_u16;
        app.diff.diff_preview_cache_key = None;
        if has_diffs {
            schedule_diff_preview_refresh(app, Duration::from_millis(0));
        }
    }
    if app.diff.diff_preview_cache_key.is_none()
        && !app.diff.diff_preview_pending
        && !job_running(app, JobKey::DiffPreview)
        && has_diffs
    {
        schedule_diff_preview_refresh(app, Duration::from_millis(0));
    }
    if diff_preview_refresh_ready(app, now) && !job_running(app, JobKey::DiffPreview) {
        if has_diffs {
            request_diff_preview_async(app, diff_inner_width);
            dirty = true;
        } else if app.diff.diff_preview_lines != vec![Line::from("No diffs")] {
            app.diff.diff_preview_lines = vec![Line::from("No diffs")];
            dirty = true;
        }
        app.diff.diff_preview_pending = false;
        app.diff.diff_preview_next_refresh_at = None;
    }

    if clamp_scroll_offsets(app, layout) {
        dirty = true;
    }
    if update_git_activity_indicators(app, now) {
        dirty = true;
    }

    dirty
}
