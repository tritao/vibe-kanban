use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::{
    super::{
        buffer::{LogAssemblerState, ProgressKind},
        markdown::{MdSoftBreakMode, render_markdown},
    },
    push_line,
};
use crate::{
    logs::{model_params::is_model_params_system_message, types::NormalizedEntryType},
    state::{DiffTheme, LogRenderMode},
    store::logs::NormalizedContentRef,
    text::truncate_to_width,
};

mod tool_use;

pub(super) fn fnv1a64(s: &str) -> (u64, u16) {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for &b in s.as_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    (hash, s.len().min(u16::MAX as usize) as u16)
}

pub(super) fn append_normalized_entry(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    state: &mut LogAssemblerState,
    entry_idx: usize,
    entry: &serde_json::Value,
    width: usize,
    render_mode: LogRenderMode,
    diff_theme: DiffTheme,
    collapsed: bool,
) {
    fn progress_line(kind: ProgressKind, count: usize, width: usize) -> Line<'static> {
        let label = match kind {
            ProgressKind::Thinking => "thinking…",
            ProgressKind::Loading => "loading…",
        };
        let text = if count > 1 {
            format!("{label} (x{count})")
        } else {
            label.to_string()
        };

        // Keep this to a single line to make in-place updates easy.
        let max = width.saturating_sub(2).max(1);
        let text = truncate_to_width(&text, max);
        Line::from(vec![
            Span::styled("▌", Style::default().add_modifier(Modifier::DIM)),
            Span::raw(" "),
            Span::styled(text, Style::default().add_modifier(Modifier::DIM)),
        ])
    }

    let content = NormalizedContentRef::new(entry);
    let entry_type_ref = match content.entry_type() {
        Some(v) => v,
        None => {
            let fallback = entry.to_string();
            push_line(lines, map, entry_idx, Line::from(fallback), width);
            return;
        }
    };

    let entry_type_tag = entry_type_ref.tag();
    let content_text = content.content_text();

    match NormalizedEntryType::parse(entry_type_tag) {
        NormalizedEntryType::UserMessage => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "You",
                crate::ui::palette::log_accent_user(),
                content_text,
                width,
                render_mode,
            );
        }
        NormalizedEntryType::AssistantMessage => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "Assistant",
                crate::ui::palette::log_accent_assistant(),
                content_text,
                width,
                render_mode,
            );
        }
        NormalizedEntryType::SystemMessage => {
            let trimmed = content_text.trim();
            if trimmed.is_empty() {
                return;
            }
            // Move model/effort system messages into the run header metadata line.
            if is_model_params_system_message(trimmed) {
                return;
            }
            let (h, len) = fnv1a64(trimmed);
            if state.has_last_system && state.last_system_hash == h && state.last_system_len == len
            {
                return;
            }
            state.has_last_system = true;
            state.last_system_hash = h;
            state.last_system_len = len;
            append_text_block(
                lines,
                map,
                entry_idx,
                "System",
                crate::ui::palette::log_accent_system(),
                trimmed,
                width,
                render_mode,
            );
        }
        NormalizedEntryType::ErrorMessage => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "Error",
                crate::ui::palette::log_accent_error(),
                content_text,
                width,
                render_mode,
            );
        }
        NormalizedEntryType::UserFeedback => {
            let denied_tool = entry_type_ref.denied_tool().unwrap_or("tool");
            append_text_block(
                lines,
                map,
                entry_idx,
                &format!("Feedback (denied {denied_tool})"),
                crate::ui::palette::log_accent_feedback(),
                content_text,
                width,
                render_mode,
            );
        }
        NormalizedEntryType::Thinking => {
            let kind = ProgressKind::Thinking;
            if state.progress_kind == Some(kind)
                && let Some(pos) = state.progress_line_pos
                && pos < lines.len()
            {
                state.progress_count = state.progress_count.saturating_add(1);
                lines[pos] = progress_line(kind, state.progress_count, width);
                map[pos] = entry_idx;
            } else {
                state.progress_kind = Some(kind);
                state.progress_count = 1;
                state.progress_line_pos = Some(lines.len());
                push_line(lines, map, entry_idx, progress_line(kind, 1, width), width);
            }
        }
        NormalizedEntryType::Loading => {
            let kind = ProgressKind::Loading;
            if state.progress_kind == Some(kind)
                && let Some(pos) = state.progress_line_pos
                && pos < lines.len()
            {
                state.progress_count = state.progress_count.saturating_add(1);
                lines[pos] = progress_line(kind, state.progress_count, width);
                map[pos] = entry_idx;
            } else {
                state.progress_kind = Some(kind);
                state.progress_count = 1;
                state.progress_line_pos = Some(lines.len());
                push_line(lines, map, entry_idx, progress_line(kind, 1, width), width);
            }
        }
        NormalizedEntryType::NextAction => {
            let failed = entry_type_ref.next_action_failed();
            let needs_setup = entry_type_ref.next_action_needs_setup();
            let procs = entry_type_ref.next_action_execution_processes();
            let text = format!(
                "next action{} (execs: {}, setup: {})",
                if failed { " (failed)" } else { "" },
                procs,
                if needs_setup { "needed" } else { "ok" }
            );
            push_line(
                lines,
                map,
                entry_idx,
                Line::from(Span::styled(
                    text,
                    Style::default().add_modifier(Modifier::DIM),
                )),
                width,
            );
        }
        NormalizedEntryType::ToolUse => {
            tool_use::append_tool_use(
                lines,
                map,
                state,
                entry_idx,
                &entry_type_ref,
                content_text,
                width,
                render_mode,
                diff_theme,
                collapsed,
            );
        }
        NormalizedEntryType::Other => {
            // Fallback: preserve existing behavior.
            let fallback = entry_type_tag.replace('_', " ");
            if !content_text.trim().is_empty() {
                append_text_block(
                    lines,
                    map,
                    entry_idx,
                    &fallback,
                    crate::ui::palette::log_accent_system(),
                    content_text,
                    width,
                    render_mode,
                );
            } else if let Some(text) = normalized_entry_text(entry) {
                let mut rendered = if render_mode == LogRenderMode::Markdown {
                    render_markdown(&text, width.max(1), MdSoftBreakMode::Newline)
                } else {
                    text.lines()
                        .map(|l| Line::from(Span::raw(l.to_string())))
                        .collect()
                };
                if rendered.is_empty() {
                    rendered.push(Line::from(""));
                }
                for l in rendered {
                    push_line(lines, map, entry_idx, l, width);
                }
            }
        }
    }
}

pub(super) fn append_text_block(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    entry_idx: usize,
    label: &str,
    accent: Color,
    text: &str,
    width: usize,
    render_mode: LogRenderMode,
) {
    let header = Line::from(vec![
        Span::styled("▌", Style::default().fg(accent)),
        Span::raw(" "),
        Span::styled(
            label.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]);
    push_line(lines, map, entry_idx, header, width);

    if text.trim().is_empty() {
        return;
    }

    let body_lines = if render_mode == LogRenderMode::Markdown {
        render_markdown(
            text,
            width.saturating_sub(2).max(1),
            MdSoftBreakMode::Newline,
        )
    } else {
        text.lines()
            .map(|l| Line::from(Span::raw(l.to_string())))
            .collect()
    };
    for l in body_lines {
        let mut spans = vec![Span::styled(
            "  ",
            Style::default().add_modifier(Modifier::DIM),
        )];
        spans.extend(l.spans.into_iter());
        push_line(lines, map, entry_idx, Line::from(spans), width);
    }
}

pub(super) fn normalized_entry_text(entry: &serde_json::Value) -> Option<String> {
    let content = NormalizedContentRef::new(entry);
    let content_text = content.content_text().to_string();
    if !content_text.trim().is_empty() {
        return Some(content_text);
    }

    let entry_type = content.entry_type()?;
    let ty = entry_type.tag();
    let kind = NormalizedEntryType::parse(ty);
    let label = match kind {
        NormalizedEntryType::ToolUse => {
            let tool = entry_type.tool_name().unwrap_or("tool");
            let status = entry_type.tool_status_str().unwrap_or("created");
            format!("{tool} ({status})")
        }
        NormalizedEntryType::NextAction => "next action".to_string(),
        NormalizedEntryType::Loading => "loading…".to_string(),
        NormalizedEntryType::Thinking => "thinking…".to_string(),
        _ => ty.replace('_', " "),
    };

    Some(label)
}
