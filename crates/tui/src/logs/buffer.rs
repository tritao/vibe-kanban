use std::collections::HashMap;

use anyhow::Context;
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use uuid::Uuid;

use crate::{
    events::StreamStatus,
    fmt::short_time,
    selection::exec_list,
    state::{AppState, DiffTheme, ExecRow, LogMode, LogRenderMode, LogViewMode},
};

fn ensure_log_store_entries_array(store: &mut serde_json::Value) {
    if !store.is_object() {
        *store = serde_json::json!({});
    }
    let obj = store.as_object_mut().expect("object");
    if !obj.get("entries").is_some_and(|v| v.is_array()) {
        obj.insert("entries".to_string(), serde_json::json!([]));
    }
}

fn parse_entries_index_and_suffix(path: &str) -> Option<(usize, &str)> {
    let rest = path.strip_prefix("/entries/")?;
    let (idx, suffix) = match rest.split_once('/') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };
    let i = idx.parse::<usize>().ok()?;
    let suffix = suffix.map(|s| {
        // Re-add leading slash so callers can reuse it as a JSON pointer suffix.
        // SAFETY: stored for the duration of this function call chain.
        Box::leak(format!("/{s}").into_boxed_str()) as &str
    });
    Some((i, suffix.unwrap_or("")))
}

fn apply_log_patch_resilient(
    store: &mut serde_json::Value,
    patch: &json_patch::Patch,
) -> anyhow::Result<()> {
    ensure_log_store_entries_array(store);

    if json_patch::patch(store, patch).is_ok() {
        return Ok(());
    }

    use json_patch::{AddOperation, PatchOperation, RemoveOperation, ReplaceOperation};

    for op in patch.iter() {
        match op {
            PatchOperation::Add(AddOperation { path, value }) => {
                let p = path.to_string();
                if let Some((idx, suffix)) = parse_entries_index_and_suffix(&p) {
                    ensure_log_store_entries_array(store);
                    let entries = store
                        .get_mut("entries")
                        .and_then(|v| v.as_array_mut())
                        .expect("entries array");
                    if suffix.is_empty() {
                        let v = value.clone();
                        if idx <= entries.len() {
                            entries.insert(idx, v);
                        } else {
                            entries.push(v);
                        }
                        continue;
                    }
                    if idx < entries.len() {
                        let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
                    }
                    continue;
                }
                let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
            }
            PatchOperation::Replace(ReplaceOperation { path, value }) => {
                let p = path.to_string();
                if let Some((idx, suffix)) = parse_entries_index_and_suffix(&p) {
                    ensure_log_store_entries_array(store);
                    let entries = store
                        .get_mut("entries")
                        .and_then(|v| v.as_array_mut())
                        .expect("entries array");
                    if suffix.is_empty() {
                        let v = value.clone();
                        if idx < entries.len() {
                            entries[idx] = v;
                        } else if idx == entries.len() {
                            entries.push(v);
                        } else {
                            // Missing history: ignore.
                        }
                        continue;
                    }
                    if idx < entries.len() {
                        let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
                    }
                    continue;
                }
                let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
            }
            PatchOperation::Remove(RemoveOperation { path }) => {
                let p = path.to_string();
                if let Some((idx, suffix)) = parse_entries_index_and_suffix(&p) {
                    ensure_log_store_entries_array(store);
                    let entries = store
                        .get_mut("entries")
                        .and_then(|v| v.as_array_mut())
                        .expect("entries array");
                    if suffix.is_empty() {
                        if idx < entries.len() {
                            entries.remove(idx);
                        }
                        continue;
                    }
                    if idx < entries.len() {
                        let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
                    }
                    continue;
                }
                let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
            }
            _ => {
                let _ = json_patch::patch(store, &json_patch::Patch(vec![op.clone()]));
            }
        }
    }

    Ok(())
}

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
    pub(crate) last_system_hash: u64,
    pub(crate) last_system_len: u16,
    pub(crate) has_last_system: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LogSelection {
    pub(crate) exec_id: Uuid,
    pub(crate) entry_idx: usize,
}

#[derive(Debug, Clone)]
struct RenderCache {
    lines: Vec<Line<'static>>,
    line_entry_index: Vec<usize>,
    entry_line_starts: Vec<usize>,
    entry_end_states: Vec<LogAssemblerState>,
    assembler_state: LogAssemblerState,
    dirty_from_entry: Option<usize>,
}

impl RenderCache {
    fn new(_width: u16) -> Self {
        Self {
            lines: vec![],
            line_entry_index: vec![],
            entry_line_starts: vec![],
            entry_end_states: vec![],
            assembler_state: LogAssemblerState::default(),
            dirty_from_entry: Some(0),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedLogCache {
    pub(crate) lines: Vec<Line<'static>>,
    pub(crate) line_entry_index: Vec<usize>,
    pub(crate) entry_line_starts: Vec<usize>,
    pub(crate) entry_end_states: Vec<LogAssemblerState>,
    pub(crate) assembler_state: LogAssemblerState,
}

impl PreparedLogCache {
    fn into_render_cache(self) -> RenderCache {
        RenderCache {
            lines: self.lines,
            line_entry_index: self.line_entry_index,
            entry_line_starts: self.entry_line_starts,
            entry_end_states: self.entry_end_states,
            assembler_state: self.assembler_state,
            dirty_from_entry: None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ExecLogBuffer {
    pub(crate) store: serde_json::Value,
    pub(crate) pending_patch: json_patch::Patch,
    pub(crate) collapsed: Vec<bool>,
    render_caches: HashMap<u16, RenderCache>,
    render_cache_lru: Vec<u16>,
}

impl Default for ExecLogBuffer {
    fn default() -> Self {
        Self {
            store: serde_json::Value::Null,
            pending_patch: Default::default(),
            collapsed: vec![],
            render_caches: HashMap::new(),
            render_cache_lru: vec![],
        }
    }
}

impl ExecLogBuffer {
    fn touch_cache_key(&mut self, width: u16) {
        if let Some(pos) = self.render_cache_lru.iter().position(|w| *w == width) {
            self.render_cache_lru.remove(pos);
        }
        self.render_cache_lru.push(width);
        const MAX_CACHES: usize = 2;
        while self.render_cache_lru.len() > MAX_CACHES {
            let evict = self.render_cache_lru.remove(0);
            self.render_caches.remove(&evict);
        }
    }

    fn cache(&self, width: u16) -> Option<&RenderCache> {
        self.render_caches.get(&width)
    }

    fn any_cache(&self) -> Option<&RenderCache> {
        self.render_cache_lru
            .last()
            .and_then(|w| self.render_caches.get(w))
            .or_else(|| self.render_caches.values().next())
    }

    pub(crate) fn cache_exists(&self, width: u16) -> bool {
        self.render_caches.contains_key(&width)
    }

    pub(crate) fn install_cache(&mut self, width: u16, cache: PreparedLogCache) {
        self.touch_cache_key(width);
        self.render_caches.insert(width, cache.into_render_cache());
    }

    pub(crate) fn ensure_init(&mut self) {
        if self.store.is_null() {
            self.store = serde_json::json!({ "entries": [] });
        }
    }

    pub(crate) fn reset(&mut self) {
        self.store = serde_json::json!({ "entries": [] });
        self.pending_patch.0.clear();
        self.collapsed.clear();
        self.render_caches.clear();
        self.render_cache_lru.clear();
    }

    pub(crate) fn enqueue_patch(&mut self, patch: json_patch::Patch) {
        self.ensure_init();
        if let Some(min_idx) = log_patch_min_entry_index(&patch) {
            for c in self.render_caches.values_mut() {
                c.dirty_from_entry = Some(
                    c.dirty_from_entry
                        .map(|v| v.min(min_idx))
                        .unwrap_or(min_idx),
                );
            }
        }
        self.pending_patch.0.extend(patch.0);
    }

    pub(crate) fn mark_dirty_from(&mut self, entry_idx: usize) {
        for c in self.render_caches.values_mut() {
            c.dirty_from_entry = Some(
                c.dirty_from_entry
                    .map(|v| v.min(entry_idx))
                    .unwrap_or(entry_idx),
            );
        }
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
        let width_u16 = width.max(1).min(u16::MAX as usize) as u16;
        let cache_missing = !self.render_caches.contains_key(&width_u16);
        let has_dirty = self
            .render_caches
            .get(&width_u16)
            .is_some_and(|c| c.dirty_from_entry.is_some());
        if !has_patch && !has_dirty && !cache_missing {
            return Ok(false);
        }

        let width = width.max(1);

        if has_patch {
            let patch = std::mem::take(&mut self.pending_patch);
            apply_log_patch_resilient(&mut self.store, &patch).context("apply patch")?;
        } else {
            self.pending_patch.0.clear();
        }

        {
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
        }

        self.touch_cache_key(width_u16);
        if cache_missing {
            self.render_caches
                .insert(width_u16, RenderCache::new(width_u16));
        }
        let cache = self
            .render_caches
            .get_mut(&width_u16)
            .expect("render cache");
        let processed_entries_before = cache.entry_line_starts.len();
        let rebuild_from = cache
            .dirty_from_entry
            .take()
            .unwrap_or(processed_entries_before);

        let rebuild_from = rebuild_from.min(processed_entries_before);
        if rebuild_from == 0 {
            cache.lines.clear();
            cache.line_entry_index.clear();
            cache.entry_line_starts.clear();
            cache.entry_end_states.clear();
            cache.assembler_state = LogAssemblerState::default();
        } else if rebuild_from < processed_entries_before {
            let truncate_to = cache.entry_line_starts[rebuild_from];
            cache.lines.truncate(truncate_to);
            cache.line_entry_index.truncate(truncate_to);
            cache.entry_line_starts.truncate(rebuild_from);
            cache.entry_end_states.truncate(rebuild_from);
            cache.assembler_state = cache.entry_end_states.last().copied().unwrap_or_default();
        }

        let entries = self
            .store
            .get("entries")
            .and_then(|v| v.as_array())
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        for idx in rebuild_from..entries.len() {
            let entry = &entries[idx];
            cache.entry_line_starts.push(cache.lines.len());
            super::assemble::append_log_entry(
                &mut cache.lines,
                &mut cache.line_entry_index,
                &mut cache.assembler_state,
                idx,
                entry,
                width,
                log_mode,
                render_mode,
                diff_theme,
                &self.collapsed,
            );
            cache.entry_end_states.push(cache.assembler_state);
        }

        Ok(true)
    }

    pub(crate) fn rendered_entry_text(&self, entry_idx: usize, width: u16) -> Option<String> {
        let cache = self.cache(width).or_else(|| self.any_cache())?;
        if entry_idx >= cache.entry_line_starts.len() {
            return None;
        }
        let start = cache.entry_line_starts[entry_idx];
        let end = cache
            .entry_line_starts
            .get(entry_idx + 1)
            .copied()
            .unwrap_or(cache.lines.len());
        let slice = cache.lines.get(start..end).unwrap_or(&[]);
        Some(crate::util::lines_plain_text(slice))
    }
}

pub(crate) fn build_prepared_log_cache(
    store: &serde_json::Value,
    collapsed: &[bool],
    width: usize,
    log_mode: LogMode,
    render_mode: LogRenderMode,
    diff_theme: DiffTheme,
) -> PreparedLogCache {
    let width = width.max(1);
    let mut out = PreparedLogCache {
        lines: vec![],
        line_entry_index: vec![],
        entry_line_starts: vec![],
        entry_end_states: vec![],
        assembler_state: LogAssemblerState::default(),
    };

    let entries = store
        .get("entries")
        .and_then(|v| v.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    for (idx, entry) in entries.iter().enumerate() {
        out.entry_line_starts.push(out.lines.len());
        super::assemble::append_log_entry(
            &mut out.lines,
            &mut out.line_entry_index,
            &mut out.assembler_state,
            idx,
            entry,
            width,
            log_mode,
            render_mode,
            diff_theme,
            collapsed,
        );
        out.entry_end_states.push(out.assembler_state);
    }

    out
}

pub(crate) fn append_local_user_message(app: &mut AppState, exec_id: Uuid, message: &str) {
    let msg = message.trim_end_matches('\n');
    if msg.trim().is_empty() {
        return;
    }

    if !app.exec.log_exec_order.contains(&exec_id) {
        app.exec.log_exec_order.push(exec_id);
    }

    let buf = app.exec.log_buffers.entry(exec_id).or_default();
    buf.ensure_init();
    if !buf
        .store
        .get("entries")
        .and_then(|v| v.as_array())
        .is_some()
    {
        buf.store = serde_json::json!({ "entries": [] });
    }
    let entries = buf
        .store
        .get_mut("entries")
        .and_then(|v| v.as_array_mut())
        .expect("entries array");

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
    let mut execs = exec_list(&app.exec.exec_store);
    execs.sort_by_key(|e| e.created_at.clone().unwrap_or_default());
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

    let width = app.exec.log_render_width;
    for (idx, exec_id) in include.iter().copied().enumerate() {
        let meta = exec_by_id.get(&exec_id);
        let status = meta.and_then(|e| e.status.as_deref()).unwrap_or("unknown");
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
