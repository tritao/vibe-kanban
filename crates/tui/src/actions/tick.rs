use std::time::Instant;

use ratatui::{layout::Rect, text::Line};

use crate::{
    commands::update_git_activity_indicators,
    diff_preview::{
        diff_preview_refresh_ready, request_diff_preview_async, schedule_diff_preview_refresh,
    },
    jobs::{job_running, reap_finished_jobs, replace_blocking_job},
    layout::{clamp_scroll_offsets, compute_main_layout},
    logs::flush_log_buffers,
    selection::exec_list,
    state::{AppState, JobKey},
};

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
        app.exec.log_prewarm_job_width = None;
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
        execs.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let primary_opt = app
            .exec
            .selected_exec_id
            .or_else(|| execs.last().map(|e| e.id));

        let target_width = app.exec.log_target_render_width;
        let include: Vec<uuid::Uuid> = match app.exec.log_view_mode {
            crate::state::LogViewMode::Single => primary_opt.into_iter().collect(),
            crate::state::LogViewMode::Timeline => execs.iter().map(|e| e.id).collect(),
        };

        if !job_running(app, JobKey::LogPrewarm)
            || app.exec.log_prewarm_job_width != Some(target_width)
        {
            app.exec.log_prewarm_job_width = Some(target_width);
            let generation = crate::jobs::latest::next_generation(&mut app.exec.log_prewarm_gen);
            let log_mode = app.exec.log_mode;
            let render_mode = app.exec.log_render_mode;
            let diff_theme = app.diff.diff_theme;
            let net_tx = app.net_tx.clone();

            let mut snapshot: Vec<crate::logs::buffer::LogPrewarmSnapshot> = vec![];
            for id in include.iter().copied() {
                if let Some(buf) = app.exec.log_buffers.get(&id) {
                    snapshot.push(buf.prewarm_snapshot(id));
                }
            }

            replace_blocking_job(app, JobKey::LogPrewarm, move || {
                for snap in snapshot {
                    let cache =
                        snap.build_cache(target_width as usize, log_mode, render_mode, diff_theme);
                    let _ = net_tx.blocking_send(crate::events::NetEvent::LogPrewarmReady {
                        exec_id: snap.exec_id,
                        width: target_width,
                        generation,
                        cache,
                    });
                }
            });
        }

        // If the primary execution has a cache for the target width, swap. Even if it becomes
        // dirty due to new incoming logs, the incremental rebuild is much cheaper than a full
        // rebuild on focus changes.
        if let Some(primary_id) = primary_opt {
            if let Some(buf) = app.exec.log_buffers.get(&primary_id) {
                if buf.cache_exists(target_width) {
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
    let has_diffs = crate::store::diff::DiffStore::new(&app.diff.diff_store).has_entries();
    if diff_width_changed {
        app.diff.diff_preview_cache_width = diff_inner_width_u16;
        app.diff.invalidate_diff_preview_cache();
        if has_diffs {
            schedule_diff_preview_refresh(app, crate::ui::constants::DIFF_PREVIEW_REFRESH_DELAY);
        }
    }
    if app.diff.diff_preview_cache_key.is_none()
        && !app.diff.diff_preview_pending
        && !job_running(app, JobKey::DiffPreview)
        && has_diffs
    {
        schedule_diff_preview_refresh(app, crate::ui::constants::DIFF_PREVIEW_REFRESH_DELAY);
    }
    if diff_preview_refresh_ready(app, now) && !job_running(app, JobKey::DiffPreview) {
        if has_diffs {
            request_diff_preview_async(app, diff_inner_width);
            dirty = true;
        } else if app.diff.diff_preview_lines != vec![Line::from("No diffs")] {
            app.diff.diff_preview_lines = vec![Line::from("No diffs")];
            app.diff.diff_preview_loading.stop();
            dirty = true;
        }
        app.diff.diff_preview_pending = false;
        app.diff.diff_preview_next_refresh_at = None;
    }

    // Show loading indicators only if the async job takes long enough to be noticeable.
    if app
        .diff
        .diff_preview_loading
        .tick(now, job_running(app, JobKey::DiffPreview))
    {
        dirty = true;
    }
    if app.diff.diff_preview_loading.visible()
        && app.diff.diff_preview_loading.placeholder_pending
        && (app.diff.diff_preview_lines.is_empty()
            || app.diff.diff_preview_lines == vec![Line::from("No diffs")])
    {
        app.diff.diff_preview_loading.placeholder_pending = false;
        app.diff.diff_preview_lines = vec![Line::from(ratatui::text::Span::styled(
            "Loading diff…".to_string(),
            ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::DIM),
        ))];
        dirty = true;
    }

    if clamp_scroll_offsets(app, layout) {
        dirty = true;
    }
    if update_git_activity_indicators(app, now) {
        dirty = true;
    }

    if app.diff.list_mode == crate::state::DiffListMode::Commits {
        if crate::commands::ensure_commit_preview_rendered(app, layout.diff_preview.width) {
            dirty = true;
        }
    }

    if app.diff.list_mode == crate::state::DiffListMode::Commits {
        if let Some(repo) = crate::state::repo_scope::selected_repo(app) {
            let repo_id = repo.repo_id;
            let running = job_running(app, JobKey::CommitList);
            if let Some(ind) = app.diff.commits_loading_by_repo.get_mut(&repo_id) {
                if ind.delay != crate::ui::constants::COMMIT_LIST_LOADING_INDICATOR_DELAY {
                    ind.delay = crate::ui::constants::COMMIT_LIST_LOADING_INDICATOR_DELAY;
                }
                if ind.tick(now, running) {
                    dirty = true;
                }
            }
        }
    }

    if app
        .diff
        .commit_preview_loading
        .tick(now, job_running(app, JobKey::CommitPreview))
    {
        dirty = true;
    }
    if app.diff.commit_preview_loading.visible()
        && app.diff.commit_preview_loading.placeholder_pending
    {
        app.diff.commit_preview_loading.placeholder_pending = false;
        if app.diff.commit_preview_text.is_none()
            && (app.diff.commit_preview_lines.is_empty()
                || app.diff.commit_preview_lines == vec![Line::from("No commit selected")])
        {
            app.diff.commit_preview_lines = vec![Line::from(ratatui::text::Span::styled(
                "Loading commit…".to_string(),
                ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::DIM),
            ))];
        }
        dirty = true;
    }

    dirty
}
