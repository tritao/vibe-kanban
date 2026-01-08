use std::time::{Duration, Instant};

use ratatui::text::Line;

use crate::{
    diff::{build_diff_preview_request, compute_diff_preview},
    events::NetEvent,
    jobs::{cancel_job, replace_blocking_job},
    state::{AppState, JobKey},
};

fn json_pointer_escape_segment(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

pub(crate) fn diff_patch_touches_key(patch: &json_patch::Patch, key: &str) -> bool {
    let escaped = json_pointer_escape_segment(key);
    let prefix = format!("/entries/{escaped}");
    patch.iter().any(|op| {
        let path = op.path().to_string();
        path == "/entries" || path == prefix || path.starts_with(&(prefix.clone() + "/"))
    })
}

pub(crate) fn on_diff_entries_patched(app: &mut AppState, patch: &json_patch::Patch) {
    use crate::diff::DIFF_ALL_KEY;

    let rows = crate::store::diff::DiffStore::new(app.diff.diff_store.as_value())
        .rows_with_all_filtered(app.diff.diff_show_untracked);
    if rows.is_empty() {
        return;
    }

    let selected_index = app
        .diff
        .selected_diff_index
        .min(rows.len().saturating_sub(1));
    let selected_key = rows
        .get(selected_index)
        .map(|r| r.key.as_str())
        .unwrap_or(DIFF_ALL_KEY);

    let should_refresh = if selected_key == DIFF_ALL_KEY {
        true
    } else {
        diff_patch_touches_key(patch, selected_key)
    };
    if !should_refresh {
        return;
    }

    app.diff.invalidate_diff_preview_cache();
    // The diff stream can send many patches during initial load (one per file). Rebuilding the
    // combined "__ALL__" preview on every patch is very expensive and looks like the view is
    // “growing” line-by-line. Debounce in ALL mode.
    if selected_key == DIFF_ALL_KEY {
        schedule_diff_preview_refresh_debounced(app, crate::ui::constants::DIFF_ALL_DEBOUNCE_DELAY);
    } else {
        schedule_diff_preview_refresh(app, crate::ui::constants::DIFF_PREVIEW_REFRESH_DELAY);
    }
}

pub(crate) fn schedule_diff_preview_refresh(app: &mut AppState, delay: Duration) {
    // Cancel any in-flight diff preview generation; a newer one will replace it.
    cancel_diff_preview_job(app);
    let now = Instant::now();
    let next = now + delay;
    app.diff.diff_preview_pending = true;
    app.diff.diff_preview_next_refresh_at = match app.diff.diff_preview_next_refresh_at {
        Some(existing) => Some(existing.min(next)),
        None => Some(next),
    };
}

pub(crate) fn schedule_diff_preview_refresh_debounced(app: &mut AppState, delay: Duration) {
    // Debounce (push the refresh further out as new patches arrive).
    cancel_diff_preview_job(app);
    let now = Instant::now();
    let next = now + delay;
    app.diff.diff_preview_pending = true;
    app.diff.diff_preview_next_refresh_at = match app.diff.diff_preview_next_refresh_at {
        Some(existing) => Some(existing.max(next)),
        None => Some(next),
    };
}

pub(crate) fn diff_preview_refresh_ready(app: &AppState, now: Instant) -> bool {
    app.diff.diff_preview_pending
        && app
            .diff
            .diff_preview_next_refresh_at
            .map(|t| now >= t)
            .unwrap_or(true)
}

pub(crate) fn cancel_diff_preview_job(app: &mut AppState) {
    app.diff.diff_preview_gen.next();
    cancel_job(app, JobKey::DiffPreview);
    app.diff.diff_preview_loading.stop();
}

pub(crate) fn request_diff_preview_async(app: &mut AppState, width: usize) {
    cancel_diff_preview_job(app);

    let generation = app.diff.diff_preview_gen.current();
    let width_u16 = (width.min(u16::MAX as usize)) as u16;
    let req = build_diff_preview_request(
        app.diff.diff_store.as_value(),
        app.diff.selected_diff_index,
        app.diff.diff_show_untracked,
    );
    let theme = app.diff.diff_theme;
    let wrap = app.diff.diff_wrap;
    let net_tx = app.net_tx.clone();

    // Avoid flicker: keep the previous preview content rendered while the async
    // rebuild runs, and only swap in the new content once ready. If there is no
    // existing content, show a single-line placeholder.
    crate::ui::loading::start_with_default_delay(
        &mut app.diff.diff_preview_loading,
        app.diff.diff_preview_lines.is_empty()
            || app.diff.diff_preview_lines
                == vec![Line::from(crate::ui::messages::placeholders::NO_DIFFS)],
    );

    replace_blocking_job(app, JobKey::DiffPreview, move || {
        let (cache_key, cache_hash, lines) = compute_diff_preview(req, width, theme, wrap);
        let _ = net_tx.blocking_send(NetEvent::DiffPreviewReady {
            generation,
            cache_key,
            cache_hash,
            width: width_u16,
            lines,
        });
    });
}
