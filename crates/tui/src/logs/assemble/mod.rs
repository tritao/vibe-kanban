mod collapse;
mod normalized;
mod stream;

pub(crate) use collapse::default_collapsed_for_log_entry;
use ratatui::{style::Style, text::Line};

use super::buffer::{LogAssemblerState, LogKind};
use crate::{
    state::{DiffTheme, LogMode, LogRenderMode},
    text::wrap_line_wordwise,
};

pub(super) fn push_line(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    entry_idx: usize,
    line: Line<'static>,
    width: usize,
) {
    for wrapped in wrap_line_wordwise(&line, width) {
        lines.push(wrapped);
        map.push(entry_idx);
    }
}

pub(crate) fn append_log_entry(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    state: &mut LogAssemblerState,
    entry_idx: usize,
    entry: &serde_json::Value,
    width: usize,
    log_mode: LogMode,
    render_mode: LogRenderMode,
    diff_theme: DiffTheme,
    collapsed: &[bool],
) {
    fn line_is_blank(line: &Line<'static>) -> bool {
        line.spans
            .iter()
            .all(|s| s.content.as_ref().trim().is_empty())
    }

    let entry_ref = crate::store::logs::LogEntryRef::new(entry);

    match entry_ref.kind() {
        crate::store::logs::LogEntryKind::Stdout => {
            let Some(text) = entry_ref.stream_text() else {
                return;
            };
            let target_idx = state.attach_to_entry.unwrap_or(entry_idx);
            if collapsed.get(target_idx).copied().unwrap_or(false) {
                return;
            }
            stream::append_stream_text(
                lines,
                map,
                state,
                LogKind::Stdout,
                text,
                Style::default(),
                true,
                target_idx,
                "  ",
                width,
            );
        }
        crate::store::logs::LogEntryKind::Stderr => {
            let Some(text) = entry_ref.stream_text() else {
                return;
            };
            let target_idx = state.attach_to_entry.unwrap_or(entry_idx);
            if collapsed.get(target_idx).copied().unwrap_or(false) {
                return;
            }
            stream::append_stream_text(
                lines,
                map,
                state,
                LogKind::Stderr,
                text,
                Style::default().fg(crate::ui::palette::log_accent_error()),
                true,
                target_idx,
                "  ",
                width,
            );
        }
        crate::store::logs::LogEntryKind::NormalizedEntry => {
            let Some(content) = entry_ref.normalized_content() else {
                return;
            };

            // Don't join stdout/stderr across normalized entries.
            state.open = false;
            state.open_kind = None;
            state.attach_to_entry = None;

            if log_mode == LogMode::Raw {
                // Raw mode intentionally focuses on stdout/stderr.
                return;
            }

            let entry_type_tag = content.entry_type().map(|t| t.tag()).unwrap_or("unknown");
            let is_progress = matches!(entry_type_tag, "thinking" | "loading");

            // Visual separation between "cards"/blocks, but don't spam blank lines for
            // ephemeral progress entries (thinking/loading).
            if is_progress {
                // If we're starting a new progress sequence (i.e. previous block wasn't a progress
                // update), add the same separation we use for other blocks.
                if state.progress_kind.is_none() {
                    if let Some(last) = lines.last() {
                        if !line_is_blank(last) {
                            let sep_owner = map.last().copied().unwrap_or(entry_idx);
                            push_line(lines, map, sep_owner, Line::from(""), width);
                        }
                    }
                }
            } else {
                // When a "real" entry arrives, stop coalescing progress.
                state.progress_kind = None;
                state.progress_count = 0;
                state.progress_line_pos = None;

                if let Some(last) = lines.last() {
                    if !line_is_blank(last) {
                        let sep_owner = map.last().copied().unwrap_or(entry_idx);
                        push_line(lines, map, sep_owner, Line::from(""), width);
                    }
                }
            }

            normalized::append_normalized_entry(
                lines,
                map,
                state,
                entry_idx,
                content,
                width,
                render_mode,
                diff_theme,
                collapsed.get(entry_idx).copied().unwrap_or(false),
            );
        }
        crate::store::logs::LogEntryKind::Other => {}
    }
}
