use std::collections::HashMap;

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use uuid::Uuid;

use super::LogSelection;
use crate::{
    events::StreamStatus,
    logs::model_params::ModelParams,
    state::{AppState, ExecRow, LogViewMode},
    store::exec_list::exec_list,
    text::truncate_to_width,
};

pub(crate) fn append_local_user_message(app: &mut AppState, exec_id: Uuid, message: &str) {
    let msg = message.trim_end_matches('\n');
    if msg.trim().is_empty() {
        return;
    }

    push_exec_order(app, app.board.selected_attempt_id, exec_id);

    let buf = app.exec.log_buffers.entry(exec_id).or_default();
    let entries = buf.store.entries_mut();

    let entry_idx = entries.len();
    entries.push(serde_json::json!({
        "type": "NORMALIZED_ENTRY",
        "content": {
            "entry_type": { "type": "user_message" },
            "content": msg,
        }
    }));
    buf.mark_dirty_from(entry_idx);
    app.exec.log_view_dirty = true;
}

pub(crate) fn set_pending_user_log(app: &mut AppState, message: String) {
    app.exec.pending_user_log_prev_exec_id = app.exec.selected_exec_id;
    app.exec.pending_user_log_wait_new_exec = true;
    app.exec.pending_user_log = Some(message);
}

pub(crate) fn maybe_attach_pending_user_log(app: &mut AppState) {
    if !app.exec.pending_user_log_wait_new_exec {
        return;
    }
    let Some(msg) = app.exec.pending_user_log.clone() else {
        app.exec.pending_user_log_wait_new_exec = false;
        return;
    };

    let prev = app.exec.pending_user_log_prev_exec_id;
    let mut execs = exec_list(app.exec.exec_store.as_value());
    execs.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let latest = execs.last().map(|e| e.id);

    let target = latest.or(app.exec.selected_exec_id);
    let Some(target) = target else {
        return;
    };

    if prev == Some(target) {
        return;
    }

    append_local_user_message(app, target, &msg);
    app.exec.pending_user_log = None;
    app.exec.pending_user_log_prev_exec_id = None;
    app.exec.pending_user_log_wait_new_exec = false;
}

pub(crate) fn mark_all_log_buffers_dirty(app: &mut AppState, entry_idx: usize) {
    for buf in app.exec.log_buffers.values_mut() {
        buf.mark_dirty_from(entry_idx);
    }
    app.exec.log_view_dirty = true;
}

fn push_exec_order(app: &mut AppState, attempt_id: Option<Uuid>, exec_id: Uuid) {
    if !app.exec.log_exec_order.contains(&exec_id) {
        app.exec.log_exec_order.push(exec_id);
    }
    let Some(attempt_id) = attempt_id else {
        return;
    };
    let list = app
        .exec
        .log_exec_order_by_attempt
        .entry(attempt_id)
        .or_default();
    if !list.contains(&exec_id) {
        list.push(exec_id);
    }
}

pub(crate) fn reset_logs(app: &mut AppState, exec_id: Option<Uuid>) {
    match exec_id {
        None => {
            // Full reset: drop cached buffers and view state.
            app.exec.log_buffers.clear();
            app.exec.log_exec_order_by_attempt.clear();
            app.exec.log_exec_order.clear();
            app.exec.log_lines.clear();
            app.exec.log_line_targets.clear();
            app.exec.log_selected = None;
            app.exec.log_scroll = crate::ui::scroll_model::ScrollFromEnd::default();
            app.exec.log_view_dirty = true;
        }
        Some(exec_id) => {
            let buf = app.exec.log_buffers.entry(exec_id).or_default();
            buf.reset();
            if app.exec.log_selected.is_some_and(|s| s.exec_id == exec_id) {
                app.exec.log_selected = None;
            }
            app.exec.log_view_dirty = true;
        }
    }
}

pub(crate) fn reset_log_view(app: &mut AppState, exec_id: Option<Uuid>) {
    // View-only reset: keep cached buffers so switching tasks/attempts doesn't "lose" logs.
    if let Some(exec_id) = exec_id {
        if app.exec.log_selected.is_some_and(|s| s.exec_id == exec_id) {
            app.exec.log_selected = None;
        }
    } else {
        app.exec.log_selected = None;
    }
    app.exec.log_lines.clear();
    app.exec.log_line_targets.clear();
    app.exec.log_scroll = crate::ui::scroll_model::ScrollFromEnd::default();
    app.exec.log_view_dirty = true;
}

fn prune_log_buffers(app: &mut AppState) {
    // Prevent unbounded growth when switching between many tasks/attempts.
    const MAX_LOG_BUFFERS: usize = 48;
    if app.exec.log_buffers.len() <= MAX_LOG_BUFFERS {
        return;
    }
    // Keep the most recently seen exec_ids (from log_exec_order).
    let mut keep: Vec<Uuid> = app.exec.log_exec_order.iter().copied().rev().collect();
    keep.dedup();
    keep.truncate(MAX_LOG_BUFFERS);

    app.exec.log_buffers.retain(|id, _| keep.contains(id));
    app.exec
        .log_exec_order
        .retain(|id| app.exec.log_buffers.contains_key(id));
}

pub(crate) fn enqueue_log_patch(
    app: &mut AppState,
    attempt_id: Option<Uuid>,
    exec_id: Uuid,
    patch: json_patch::Patch,
) {
    let mut attempt_for_order = attempt_id;
    if attempt_for_order.is_some() {
        let is_selected = app.exec.selected_exec_id == Some(exec_id);
        let in_store = exec_list(app.exec.exec_store.as_value())
            .iter()
            .any(|e| e.id == exec_id);
        if !is_selected && !in_store {
            attempt_for_order = None;
        }
    }
    push_exec_order(app, attempt_for_order, exec_id);

    let buf = app.exec.log_buffers.entry(exec_id).or_default();
    buf.enqueue_patch(patch);
    app.exec.log_view_dirty = true;

    prune_log_buffers(app);
}

fn rebuild_log_view_cache(app: &mut AppState) {
    let mut execs = exec_list(app.exec.exec_store.as_value());
    execs.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let mut ordered: Vec<Uuid> = execs.iter().map(|e| e.id).collect();
    if let Some(attempt_id) = app.board.selected_attempt_id {
        if let Some(ids) = app.exec.log_exec_order_by_attempt.get(&attempt_id) {
            for id in ids.iter().copied() {
                if !ordered.contains(&id) {
                    ordered.push(id);
                }
            }
        }
    }

    app.exec.log_lines.clear();
    app.exec.log_line_targets.clear();

    let include: Vec<Uuid> = match app.exec.log_view_mode {
        LogViewMode::Timeline => ordered.clone(),
        LogViewMode::Single => app
            .exec
            .selected_exec_id
            .or_else(|| ordered.last().copied())
            .into_iter()
            .collect(),
    };

    let exec_by_id: HashMap<Uuid, ExecRow> = execs.into_iter().map(|e| (e.id, e)).collect();

    let width = app.exec.log_render_width;
    let mut last_params: Option<ModelParams> = None;
    for (idx, exec_id) in include.iter().copied().enumerate() {
        let meta = exec_by_id.get(&exec_id);
        let status = meta
            .and_then(|e| e.status)
            .map(|s| s.label())
            .unwrap_or("unknown");
        let when = meta
            .and_then(|e| e.created_at.as_ref())
            .map(|t| t.short_time())
            .unwrap_or_default();
        let run_no = idx + 1;
        let header = if when.is_empty() {
            format!("── Run {run_no} ({status}) ──")
        } else {
            format!("── Run {run_no} ({status}) {when} ──")
        };
        app.exec.log_lines.push(Line::from(Span::styled(
            header,
            Style::default().add_modifier(Modifier::BOLD),
        )));
        app.exec.log_line_targets.push(None);

        if let Some(buf) = app.exec.log_buffers.get(&exec_id) {
            if let Some(params) =
                crate::store::logs::LogStore::new(buf.store.as_value()).model_params()
            {
                if last_params.as_ref() != Some(&params) {
                    last_params = Some(params.clone());
                    let mut text = format!("  model: {}", params.model);
                    if let Some(effort) = params.reasoning_effort.as_ref() {
                        text.push_str(&format!("  effort: {effort}"));
                    }
                    let max = width.saturating_sub(1) as usize;
                    let text = truncate_to_width(&text, max.max(1));
                    app.exec.log_lines.push(Line::from(Span::styled(
                        text,
                        Style::default().add_modifier(Modifier::DIM),
                    )));
                    app.exec.log_line_targets.push(None);
                }
            }
        }

        if let Some(buf) = app.exec.log_buffers.get(&exec_id) {
            let cache = buf.cache(width).or_else(|| buf.any_cache());
            let Some(cache) = cache else {
                app.exec.log_lines.push(Line::from(Span::styled(
                    "  (no output yet)",
                    Style::default().add_modifier(Modifier::DIM),
                )));
                app.exec.log_line_targets.push(None);
                continue;
            };

            if cache.lines.is_empty() {
                app.exec.log_lines.push(Line::from(Span::styled(
                    "  (no output yet)",
                    Style::default().add_modifier(Modifier::DIM),
                )));
                app.exec.log_line_targets.push(None);
            } else {
                for (line_idx, line) in cache.lines.iter().enumerate() {
                    app.exec.log_lines.push(line.clone());
                    let entry_idx = cache.line_entry_index.get(line_idx).copied();
                    app.exec
                        .log_line_targets
                        .push(entry_idx.map(|entry_idx| LogSelection { exec_id, entry_idx }));
                }
            }
        } else {
            app.exec.log_lines.push(Line::from(Span::styled(
                "  (no output yet)",
                Style::default().add_modifier(Modifier::DIM),
            )));
            app.exec.log_line_targets.push(None);
        }

        if idx + 1 < include.len() {
            app.exec.log_lines.push(Line::from(""));
            app.exec.log_line_targets.push(None);
        }
    }

    app.exec.log_view_dirty = false;
}

pub(crate) fn flush_log_buffers(app: &mut AppState, width: usize) -> bool {
    let prev_len = app.exec.log_lines.len();
    let mut any = false;
    let log_mode = app.exec.log_mode;
    let render_mode = app.exec.log_render_mode;
    let diff_theme = app.diff.diff_theme;

    for (_id, buf) in app.exec.log_buffers.iter_mut() {
        match buf.flush(width, log_mode, render_mode, diff_theme) {
            Ok(changed) => {
                if changed {
                    any = true;
                }
            }
            Err(e) => {
                app.ui.set_error(crate::fmt::op_failed("log patch", e));
                app.exec.log_status = StreamStatus::Error;
                any = true;
            }
        }
    }

    if any {
        app.exec.log_view_dirty = true;
    }
    if app.exec.log_view_dirty {
        rebuild_log_view_cache(app);
        let new_len = app.exec.log_lines.len();
        if app.exec.log_scroll.autoscroll {
            app.exec.log_scroll.scroll_to_end();
        } else if new_len > prev_len {
            app.exec.log_scroll.offset_from_end = app
                .exec
                .log_scroll
                .offset_from_end
                .saturating_add(new_len - prev_len);
        }
        return true;
    }
    false
}

pub(crate) fn toggle_selected_log_entry(app: &mut AppState) {
    let sel = app
        .exec
        .log_selected
        .or_else(|| app.exec.log_line_targets.iter().rev().find_map(|t| *t));
    let Some(mut sel) = sel else {
        return;
    };

    let Some(buf) = app.exec.log_buffers.get_mut(&sel.exec_id) else {
        return;
    };
    let entries = buf.store.entries();
    if entries.is_empty() {
        return;
    }

    if sel.entry_idx >= entries.len() {
        sel.entry_idx = entries.len().saturating_sub(1);
    }

    if buf.collapsed.len() < entries.len() {
        let before = buf.collapsed.len();
        buf.collapsed.resize(entries.len(), false);
        for idx in before..entries.len() {
            buf.collapsed[idx] =
                crate::logs::assemble::default_collapsed_for_log_entry(&entries[idx]);
        }
    }

    if let Some(v) = buf.collapsed.get_mut(sel.entry_idx) {
        *v = !*v;
    }
    buf.mark_dirty_from(sel.entry_idx);
    app.exec.log_selected = Some(sel);
    app.exec.log_view_dirty = true;
}
