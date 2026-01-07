use std::collections::HashMap;

use anyhow::Context;
use ratatui::text::Line;
use uuid::Uuid;

use crate::state::{DiffTheme, LogMode, LogRenderMode};

mod patch;
mod view;

pub(crate) use view::{
    append_local_user_message, enqueue_log_patch, flush_log_buffers, mark_all_log_buffers_dirty,
    maybe_attach_pending_user_log, reset_log_view, reset_logs, set_pending_user_log,
    toggle_selected_log_entry,
};

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
    pub(crate) last_todos_hash: u64,
    pub(crate) last_todos_len: u16,
    pub(crate) has_last_todos: bool,
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
        if let Some(min_idx) = patch::log_patch_min_entry_index(&patch) {
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
            patch::apply_log_patch_resilient(&mut self.store, &patch).context("apply patch")?;
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
