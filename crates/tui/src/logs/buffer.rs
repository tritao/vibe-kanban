use std::collections::HashMap;

use anyhow::Context;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use uuid::Uuid;

use crate::events::StreamStatus;
use crate::fmt::short_time;
use crate::selection::exec_list;
use crate::state::{AppState, DiffTheme, ExecRow, LogMode, LogRenderMode, LogViewMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogKind {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProgressKind {
    Thinking,
    Loading,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LogAssemblerState {
    pub(crate) open: bool,
    pub(crate) open_kind: Option<LogKind>,
    pub(crate) attach_to_entry: Option<usize>,
    pub(crate) progress_kind: Option<ProgressKind>,
    pub(crate) progress_count: usize,
    pub(crate) progress_line_pos: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LogSelection {
    pub(crate) exec_id: Uuid,
    pub(crate) entry_idx: usize,
}

#[derive(Debug, Default)]
pub(crate) struct ExecLogBuffer {
    pub(crate) store: serde_json::Value,
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) line_entry_index: Vec<usize>,
    pub(crate) entry_line_starts: Vec<usize>,
    pub(crate) entry_end_states: Vec<LogAssemblerState>,
    pub(crate) assembler_state: LogAssemblerState,
    pub(crate) pending_patch: json_patch::Patch,
    pub(crate) pending_dirty_from_entry: Option<usize>,
    pub(crate) collapsed: Vec<bool>,
}

impl ExecLogBuffer {
    pub(crate) fn ensure_init(&mut self) {
        if self.store.is_null() {
            self.store = serde_json::json!({ "entries": [] });
        }
    }

    pub(crate) fn reset(&mut self) {
        self.store = serde_json::json!({ "entries": [] });
        self.lines.clear();
        self.line_entry_index.clear();
        self.entry_line_starts.clear();
        self.entry_end_states.clear();
        self.assembler_state = LogAssemblerState::default();
        self.pending_patch.0.clear();
        self.pending_dirty_from_entry = None;
        self.collapsed.clear();
    }

    pub(crate) fn enqueue_patch(&mut self, patch: json_patch::Patch) {
        self.ensure_init();
        if let Some(min_idx) = log_patch_min_entry_index(&patch) {
            self.pending_dirty_from_entry = Some(
                self.pending_dirty_from_entry
                    .map(|v| v.min(min_idx))
                    .unwrap_or(min_idx),
            );
        }
        self.pending_patch.0.extend(patch.0);
    }

    pub(crate) fn mark_dirty_from(&mut self, entry_idx: usize) {
        self.pending_dirty_from_entry = Some(
            self.pending_dirty_from_entry
                .map(|v| v.min(entry_idx))
                .unwrap_or(entry_idx),
        );
    }

    pub(crate) fn flush(
        &mut self,
        width: usize,
        log_mode: LogMode,
        render_mode: LogRenderMode,
        diff_theme: DiffTheme,
    ) -> anyhow::Result<bool> {
        self.ensure_init();

        let has_patch = !self.pending_patch.0.is_empty();
        let has_dirty = self.pending_dirty_from_entry.is_some();
        if !has_patch && !has_dirty {
            return Ok(false);
        }

        let width = width.max(1);
        let processed_entries_before = self.entry_line_starts.len();
        let rebuild_from = self
            .pending_dirty_from_entry
            .take()
            .unwrap_or(processed_entries_before);

        if has_patch {
            let patch = std::mem::take(&mut self.pending_patch);
            json_patch::patch(&mut self.store, &patch).context("apply patch")?;
        } else {
            self.pending_patch.0.clear();
        }

        let entries = self
            .store
            .get("entries")
            .and_then(|v| v.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        if self.collapsed.len() > entries.len() {
            self.collapsed.truncate(entries.len());
        }
        if self.collapsed.len() < entries.len() {
            let before = self.collapsed.len();
            self.collapsed.resize(entries.len(), false);
            for idx in before..entries.len() {
                self.collapsed[idx] =
                    super::assemble::default_collapsed_for_log_entry(&entries[idx]);
            }
        }

        let rebuild_from = rebuild_from.min(processed_entries_before);
        if rebuild_from == 0 {
            self.lines.clear();
            self.line_entry_index.clear();
            self.entry_line_starts.clear();
            self.entry_end_states.clear();
            self.assembler_state = LogAssemblerState::default();
        } else if rebuild_from < processed_entries_before {
            let truncate_to = self.entry_line_starts[rebuild_from];
            self.lines.truncate(truncate_to);
            self.line_entry_index.truncate(truncate_to);
            self.entry_line_starts.truncate(rebuild_from);
            self.entry_end_states.truncate(rebuild_from);
            self.assembler_state = self.entry_end_states.last().copied().unwrap_or_default();
        }

        for idx in rebuild_from..entries.len() {
            let entry = &entries[idx];
            self.entry_line_starts.push(self.lines.len());
            super::assemble::append_log_entry(
                &mut self.lines,
                &mut self.line_entry_index,
                &mut self.assembler_state,
                idx,
                entry,
                width,
                log_mode,
                render_mode,
                diff_theme,
                &self.collapsed,
            );
            self.entry_end_states.push(self.assembler_state);
        }

        Ok(true)
    }

}

pub(crate) fn mark_all_log_buffers_dirty(app: &mut AppState, entry_idx: usize) {
    for buf in app.exec.log_buffers.values_mut() {
        buf.mark_dirty_from(entry_idx);
    }
    app.exec.log_view_dirty = true;
}

pub(crate) fn reset_logs(app: &mut AppState, exec_id: Option<Uuid>) {
    match exec_id {
        None => {
            app.exec.log_buffers.clear();
            app.exec.log_exec_order.clear();
            app.exec.log_lines.clear();
            app.exec.log_line_targets.clear();
            app.exec.log_selected = None;
            app.exec.log_autoscroll = true;
            app.exec.log_scroll_offset = 0;
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

pub(crate) fn enqueue_log_patch(app: &mut AppState, exec_id: Uuid, patch: json_patch::Patch) {
    if !app.exec.log_exec_order.contains(&exec_id) {
        app.exec.log_exec_order.push(exec_id);
    }

    let buf = app.exec.log_buffers.entry(exec_id).or_default();
    buf.enqueue_patch(patch);
    app.exec.log_view_dirty = true;
}

fn rebuild_log_view_cache(app: &mut AppState) {
    let mut execs = exec_list(&app.exec.exec_store);
    execs.sort_by_key(|e| e.created_at.clone().unwrap_or_default());

    let mut ordered: Vec<Uuid> = execs.iter().map(|e| e.id).collect();
    for id in app.exec.log_exec_order.iter().copied() {
        if !ordered.contains(&id) {
            ordered.push(id);
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

    for (idx, exec_id) in include.iter().copied().enumerate() {
        let meta = exec_by_id.get(&exec_id);
        let status = meta
            .and_then(|e| e.status.as_deref())
            .unwrap_or("unknown");
        let when = meta
            .and_then(|e| e.created_at.as_deref())
            .and_then(short_time)
            .unwrap_or("");
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
            if buf.lines.is_empty() {
                app.exec.log_lines.push(Line::from(Span::styled(
                    "  (no output yet)",
                    Style::default().add_modifier(Modifier::DIM),
                )));
                app.exec.log_line_targets.push(None);
            } else {
                for (line_idx, line) in buf.lines.iter().enumerate() {
                    app.exec.log_lines.push(line.clone());
                    let entry_idx = buf.line_entry_index.get(line_idx).copied();
                    app.exec
                        .log_line_targets
                        .push(entry_idx.map(|entry_idx| LogSelection {
                        exec_id,
                        entry_idx,
                    }));
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
                app.ui.last_error = Some(format!("failed to apply log patch: {e}"));
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
        if app.exec.log_autoscroll {
            app.exec.log_scroll_offset = 0;
        } else if new_len > prev_len {
            app.exec.log_scroll_offset = app
                .exec
                .log_scroll_offset
                .saturating_add(new_len - prev_len);
        }
        return true;
    }
    false
}

fn log_patch_min_entry_index(patch: &json_patch::Patch) -> Option<usize> {
    patch
        .iter()
        .filter_map(|op| {
            let path = op.path().to_string();
            let rest = path.strip_prefix("/entries/")?;
            let idx = rest.split('/').next()?;
            idx.parse::<usize>().ok()
        })
        .min()
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
    buf.ensure_init();
    let entries = buf
        .store
        .get("entries")
        .and_then(|v| v.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
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
            buf.collapsed[idx] = super::assemble::default_collapsed_for_log_entry(&entries[idx]);
        }
    }

    if let Some(v) = buf.collapsed.get_mut(sel.entry_idx) {
        *v = !*v;
    }
    buf.mark_dirty_from(sel.entry_idx);
    app.exec.log_selected = Some(sel);
    app.exec.log_view_dirty = true;
}

// Keep log-kind specific styling in the assembler; we currently only render stdout/stderr.
