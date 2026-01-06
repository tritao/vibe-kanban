use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::{
    buffer::{LogAssemblerState, LogKind, ProgressKind},
    markdown::{MdSoftBreakMode, render_markdown},
};
use crate::{
    diff::highlight_unified_diff,
    logs::model_params::is_model_params_system_message,
    state::{DiffTheme, LogMode, LogRenderMode},
    text::{line_display_width, sanitize_tui_text, truncate_to_width, wrap_line_wordwise},
};

pub(crate) fn default_collapsed_for_log_entry(entry: &serde_json::Value) -> bool {
    const THRESHOLD_LINES: usize = 24;

    let Some(ty) = entry.get("type").and_then(|v| v.as_str()) else {
        return false;
    };
    if ty != "NORMALIZED_ENTRY" {
        return false;
    }

    let Some(content) = entry.get("content") else {
        return false;
    };
    let Some(entry_type) = content.get("entry_type") else {
        return false;
    };
    let Some(entry_type_tag) = entry_type.get("type").and_then(|v| v.as_str()) else {
        return false;
    };
    if entry_type_tag != "tool_use" {
        return false;
    }

    let Some(action_type) = entry_type.get("action_type") else {
        return false;
    };
    let Some(action) = action_type.get("action").and_then(|v| v.as_str()) else {
        return false;
    };

    match action {
        "command_run" => {
            let output = action_type
                .get("result")
                .and_then(|v| v.get("output"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            output.lines().count() > THRESHOLD_LINES
        }
        "file_edit" => {
            let mut lines = 0usize;
            let changes = action_type.get("changes").and_then(|v| v.as_array());
            for c in changes.into_iter().flatten() {
                if c.get("action").and_then(|v| v.as_str()) == Some("edit") {
                    let diff = c.get("unified_diff").and_then(|v| v.as_str()).unwrap_or("");
                    lines = lines.saturating_add(diff.lines().count());
                    if lines > THRESHOLD_LINES {
                        return true;
                    }
                }
            }
            false
        }
        "tool" => {
            let result_type = action_type
                .get("result")
                .and_then(|v| v.get("type"))
                .and_then(|v| v.as_str());
            let value = action_type.get("result").and_then(|v| v.get("value"));
            match (result_type, value) {
                (Some("markdown"), Some(v)) => v
                    .as_str()
                    .unwrap_or("")
                    .lines()
                    .count()
                    .gt(&THRESHOLD_LINES),
                (Some("json"), Some(v)) => serde_json::to_string_pretty(v)
                    .ok()
                    .map(|s| s.lines().count() > THRESHOLD_LINES)
                    .unwrap_or(false),
                _ => false,
            }
        }
        _ => false,
    }
}

fn tool_status_str(entry_type: &serde_json::Value) -> Option<&str> {
    let status = entry_type.get("status")?;
    if let Some(s) = status.as_str() {
        return Some(s);
    }
    status.get("status").and_then(|v| v.as_str())
}

fn tool_status_badge(status: Option<&str>) -> (Span<'static>, Style) {
    match status.unwrap_or("created") {
        "success" => (
            Span::styled("ok", Style::default().fg(Color::Green)),
            Style::default().fg(Color::Green),
        ),
        "failed" => (
            Span::styled("fail", Style::default().fg(Color::Red)),
            Style::default().fg(Color::Red),
        ),
        "denied" => (
            Span::styled("denied", Style::default().fg(Color::Yellow)),
            Style::default().fg(Color::Yellow),
        ),
        "pending_approval" => (
            Span::styled("approval", Style::default().fg(Color::Magenta)),
            Style::default().fg(Color::Magenta),
        ),
        "timed_out" => (
            Span::styled("timeout", Style::default().fg(Color::Yellow)),
            Style::default().fg(Color::Yellow),
        ),
        _ => (
            Span::styled("…", Style::default().add_modifier(Modifier::DIM)),
            Style::default().add_modifier(Modifier::DIM),
        ),
    }
}

fn push_line(
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

    let Some(ty) = entry.get("type").and_then(|v| v.as_str()) else {
        return;
    };

    match ty {
        "STDOUT" => {
            let Some(text) = entry.get("content").and_then(|v| v.as_str()) else {
                return;
            };
            let target_idx = state.attach_to_entry.unwrap_or(entry_idx);
            if collapsed.get(target_idx).copied().unwrap_or(false) {
                return;
            }
            append_stream_text(
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
        "STDERR" => {
            let Some(text) = entry.get("content").and_then(|v| v.as_str()) else {
                return;
            };
            let target_idx = state.attach_to_entry.unwrap_or(entry_idx);
            if collapsed.get(target_idx).copied().unwrap_or(false) {
                return;
            }
            append_stream_text(
                lines,
                map,
                state,
                LogKind::Stderr,
                text,
                Style::default().fg(Color::Red),
                true,
                target_idx,
                "  ",
                width,
            );
        }
        "NORMALIZED_ENTRY" => {
            let Some(content) = entry.get("content") else {
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

            let entry_type_tag = content
                .get("entry_type")
                .and_then(|v| v.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
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

            append_normalized_entry(
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
        _ => {}
    }
}

fn append_stream_text(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    state: &mut LogAssemblerState,
    kind: LogKind,
    text: &str,
    style: Style,
    allow_join: bool,
    entry_idx: usize,
    prefix: &str,
    width: usize,
) {
    if text.is_empty() {
        return;
    }

    let ends_with_newline = text.ends_with('\n');
    let mut is_first = true;
    for raw in text.split_terminator('\n') {
        let seg = raw.strip_suffix('\r').unwrap_or(raw);
        let seg = sanitize_tui_text(seg);

        if allow_join
            && is_first
            && state.open
            && state.open_kind == Some(kind)
            && map.last().copied() == Some(entry_idx)
            && lines
                .last()
                .is_some_and(|l| l.spans.len() == 1 && l.spans[0].style == style)
        {
            if let Some(last) = lines.last_mut() {
                if let Some(span) = last.spans.first_mut() {
                    span.content.to_mut().push_str(seg.as_ref());
                }
            }
            // If joining caused the line to overflow, re-wrap it.
            if lines
                .last()
                .is_some_and(|l| line_display_width(l) > width.max(1))
            {
                let line = lines.pop().unwrap();
                let _ = map.pop();
                push_line(lines, map, entry_idx, line, width);
            }
        } else {
            push_line(
                lines,
                map,
                entry_idx,
                Line::from(Span::styled(format!("{prefix}{}", seg), style)),
                width,
            );
        }

        is_first = false;
    }

    if allow_join && !ends_with_newline {
        state.open = true;
        state.open_kind = Some(kind);
    } else {
        state.open = false;
        state.open_kind = None;
    }
}

fn append_normalized_entry(
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
    fn fnv1a64(s: &str) -> (u64, u16) {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        let mut hash = FNV_OFFSET;
        for &b in s.as_bytes() {
            hash ^= b as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        (hash, s.len().min(u16::MAX as usize) as u16)
    }
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

    fn append_text_block(
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

    let entry_type = entry.get("entry_type");
    let entry_type = match entry_type {
        Some(v) => v,
        None => {
            let fallback = entry.to_string();
            push_line(lines, map, entry_idx, Line::from(fallback), width);
            return;
        }
    };

    let entry_type_tag = entry_type
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let content_text = entry.get("content").and_then(|v| v.as_str()).unwrap_or("");

    match entry_type_tag {
        "user_message" => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "You",
                Color::Yellow,
                content_text,
                width,
                render_mode,
            );
        }
        "assistant_message" => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "Assistant",
                Color::Cyan,
                content_text,
                width,
                render_mode,
            );
        }
        "system_message" => {
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
                Color::Gray,
                trimmed,
                width,
                render_mode,
            );
        }
        "error_message" => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "Error",
                Color::Red,
                content_text,
                width,
                render_mode,
            );
        }
        "user_feedback" => {
            let denied_tool = entry_type
                .get("denied_tool")
                .and_then(|v| v.as_str())
                .unwrap_or("tool");
            append_text_block(
                lines,
                map,
                entry_idx,
                &format!("Feedback (denied {denied_tool})"),
                Color::Yellow,
                content_text,
                width,
                render_mode,
            );
        }
        "thinking" => {
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
        "loading" => {
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
        "next_action" => {
            let failed = entry_type
                .get("failed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let needs_setup = entry_type
                .get("needs_setup")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let procs = entry_type
                .get("execution_processes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
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
        "tool_use" => {
            let status = tool_status_str(entry_type);
            let (status_badge, _status_style) = tool_status_badge(status);

            let action_type = entry_type.get("action_type");
            let action_type = match action_type {
                Some(v) => v,
                None => {
                    append_text_block(
                        lines,
                        map,
                        entry_idx,
                        "Tool",
                        Color::Blue,
                        content_text,
                        width,
                        render_mode,
                    );
                    return;
                }
            };
            let action = action_type
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("other");

            let (label, accent) = match action {
                "file_read" | "search" => ("Explored", Color::Cyan),
                "file_edit" => ("Edited", Color::Green),
                "command_run" => ("Ran", Color::Cyan),
                "web_fetch" => ("Fetched", Color::Cyan),
                "task_create" => ("Created", Color::Green),
                "plan_presentation" => ("Plan", Color::Magenta),
                "todo_management" => ("Todos", Color::Magenta),
                "tool" => ("Tool", Color::Blue),
                _ => ("Tool", Color::Blue),
            };

            let arrow = if collapsed { "▸" } else { "▾" };

            let mut header_spans: Vec<Span<'static>> = vec![
                Span::styled("▌", Style::default().fg(accent)),
                Span::raw(" "),
                Span::styled(
                    arrow.to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::raw(" "),
            ];

            // Put the most important detail in the header for scannability.
            match action {
                "command_run" => {
                    let cmd = action_type
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("command");
                    header_spans.push(Span::styled(
                        format!("{label} {cmd}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                }
                "file_edit" => {
                    let path = action_type
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("file");
                    header_spans.push(Span::styled(
                        format!("{label} {path}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                }
                _ => {
                    header_spans.push(Span::styled(
                        label.to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                }
            }

            header_spans.push(Span::raw(" ("));
            header_spans.push(status_badge);
            header_spans.push(Span::raw(")"));
            push_line(lines, map, entry_idx, Line::from(header_spans), width);

            match action {
                "file_read" => {
                    let path = action_type
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("file");
                    push_line(
                        lines,
                        map,
                        entry_idx,
                        Line::from(vec![
                            Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                            Span::raw(format!("Read {path}")),
                        ]),
                        width,
                    );
                }
                "search" => {
                    let query = action_type
                        .get("query")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    push_line(
                        lines,
                        map,
                        entry_idx,
                        Line::from(vec![
                            Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                            Span::raw(format!("Search {query}")),
                        ]),
                        width,
                    );
                }
                "command_run" => {
                    state.attach_to_entry = Some(entry_idx);

                    let exit_status = action_type.get("result").and_then(|v| v.get("exit_status"));
                    if let Some(es) = exit_status {
                        let code = es
                            .get("code")
                            .and_then(|v| v.as_i64())
                            .or_else(|| es.as_i64())
                            .or_else(|| es.as_u64().map(|v| v as i64));
                        if let Some(code) = code
                            && code != 0
                        {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  - ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        format!("exit {code}"),
                                        Style::default().fg(Color::Red),
                                    ),
                                ]),
                                width,
                            );
                        }
                    }

                    let output = action_type
                        .get("result")
                        .and_then(|v| v.get("output"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !output.is_empty() {
                        if collapsed {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  ▸ ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        "output (collapsed)".to_string(),
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                ]),
                                width,
                            );
                        } else {
                            const MAX_OUTPUT_LINES: usize = 400;
                            for (i, l) in output.lines().take(MAX_OUTPUT_LINES).enumerate() {
                                let line = sanitize_tui_text(l.strip_suffix('\r').unwrap_or(l));
                                push_line(
                                    lines,
                                    map,
                                    entry_idx,
                                    Line::from(Span::styled(
                                        format!("  {}", line),
                                        Style::default().add_modifier(Modifier::DIM),
                                    )),
                                    width,
                                );
                                if i + 1 == MAX_OUTPUT_LINES {
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(Span::styled(
                                            "  … (truncated)".to_string(),
                                            Style::default().add_modifier(Modifier::DIM),
                                        )),
                                        width,
                                    );
                                }
                            }
                        }
                    }
                }
                "web_fetch" => {
                    let url = action_type
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    push_line(
                        lines,
                        map,
                        entry_idx,
                        Line::from(vec![
                            Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                            Span::raw(format!("GET {url}")),
                        ]),
                        width,
                    );
                }
                "file_edit" => {
                    let path = action_type
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("file");
                    let changes = action_type
                        .get("changes")
                        .and_then(|v| v.as_array())
                        .cloned();
                    if let Some(changes) = changes {
                        let mut has_diff = false;
                        for c in &changes {
                            let action =
                                c.get("action").and_then(|v| v.as_str()).unwrap_or("change");
                            match action {
                                "write" => {
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(vec![
                                            Span::styled(
                                                "  - ",
                                                Style::default().add_modifier(Modifier::DIM),
                                            ),
                                            Span::raw("Write content".to_string()),
                                        ]),
                                        width,
                                    );
                                }
                                "delete" => {
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(vec![
                                            Span::styled(
                                                "  - ",
                                                Style::default().add_modifier(Modifier::DIM),
                                            ),
                                            Span::raw("Delete file".to_string()),
                                        ]),
                                        width,
                                    );
                                }
                                "rename" => {
                                    let new_path = c
                                        .get("new_path")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("new path");
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(vec![
                                            Span::styled(
                                                "  - ",
                                                Style::default().add_modifier(Modifier::DIM),
                                            ),
                                            Span::raw(format!("Rename → {new_path}")),
                                        ]),
                                        width,
                                    );
                                }
                                "edit" => {
                                    has_diff = true;
                                }
                                _ => {}
                            }
                        }

                        if has_diff {
                            if collapsed {
                                push_line(
                                    lines,
                                    map,
                                    entry_idx,
                                    Line::from(vec![
                                        Span::styled(
                                            "  ▸ ",
                                            Style::default().add_modifier(Modifier::DIM),
                                        ),
                                        Span::styled(
                                            "diff (collapsed)".to_string(),
                                            Style::default().add_modifier(Modifier::DIM),
                                        ),
                                    ]),
                                    width,
                                );
                            } else {
                                const MAX_DIFF_LINES: usize = 300;
                                for c in &changes {
                                    if c.get("action").and_then(|v| v.as_str()) != Some("edit") {
                                        continue;
                                    }
                                    let diff = c
                                        .get("unified_diff")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    if diff.trim().is_empty() {
                                        continue;
                                    }
                                    let body_width = width.saturating_sub(2).max(1);
                                    let mut rendered = highlight_unified_diff(
                                        path, diff, body_width, diff_theme, false,
                                    );
                                    if rendered.len() > MAX_DIFF_LINES {
                                        rendered.truncate(MAX_DIFF_LINES);
                                        rendered.push(Line::from(Span::styled(
                                            "… (truncated)".to_string(),
                                            Style::default().add_modifier(Modifier::DIM),
                                        )));
                                    }
                                    for l in rendered {
                                        let mut spans: Vec<Span<'static>> = vec![Span::styled(
                                            "  ",
                                            Style::default().add_modifier(Modifier::DIM),
                                        )];
                                        spans.extend(l.spans.into_iter());
                                        push_line(lines, map, entry_idx, Line::from(spans), width);
                                    }
                                }
                            }
                        }
                    } else {
                        push_line(
                            lines,
                            map,
                            entry_idx,
                            Line::from(vec![
                                Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                                Span::raw(format!("Edit {path}")),
                            ]),
                            width,
                        );
                    }
                }
                "task_create" => {
                    let description = action_type
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !description.trim().is_empty() {
                        if collapsed {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  ▸ ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        "details (collapsed)".to_string(),
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                ]),
                                width,
                            );
                        } else {
                            for l in render_markdown(
                                description,
                                width.saturating_sub(2).max(1),
                                MdSoftBreakMode::Space,
                            ) {
                                let mut spans = vec![Span::styled(
                                    "  ",
                                    Style::default().add_modifier(Modifier::DIM),
                                )];
                                spans.extend(l.spans.into_iter());
                                push_line(lines, map, entry_idx, Line::from(spans), width);
                            }
                        }
                    }
                }
                "plan_presentation" => {
                    let plan = action_type
                        .get("plan")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !plan.trim().is_empty() {
                        if collapsed {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  ▸ ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        "plan (collapsed)".to_string(),
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                ]),
                                width,
                            );
                        } else {
                            for l in render_markdown(
                                plan,
                                width.saturating_sub(2).max(1),
                                MdSoftBreakMode::Space,
                            ) {
                                let mut spans = vec![Span::styled(
                                    "  ",
                                    Style::default().add_modifier(Modifier::DIM),
                                )];
                                spans.extend(l.spans.into_iter());
                                push_line(lines, map, entry_idx, Line::from(spans), width);
                            }
                        }
                    }
                }
                "todo_management" => {
                    let todos = action_type.get("todos").and_then(|v| v.as_array()).cloned();
                    if let Some(todos) = todos {
                        let count = todos.len();
                        push_line(
                            lines,
                            map,
                            entry_idx,
                            Line::from(vec![
                                Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                                Span::raw(format!("{count} todos")),
                            ]),
                            width,
                        );
                        if !collapsed {
                            for t in todos.iter().take(50) {
                                let content =
                                    t.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                let status = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
                                let mark = if status == "done" { "[x]" } else { "[ ]" };
                                push_line(
                                    lines,
                                    map,
                                    entry_idx,
                                    Line::from(vec![
                                        Span::styled(
                                            "  ",
                                            Style::default().add_modifier(Modifier::DIM),
                                        ),
                                        Span::raw(format!("{mark} {content}")),
                                    ]),
                                    width,
                                );
                            }
                        }
                    }
                }
                "tool" => {
                    let tool_name = action_type
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool");
                    let args = action_type.get("arguments");
                    if let Some(args) = args {
                        if let Ok(pretty) = serde_json::to_string_pretty(args) {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  - ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::raw(format!("{tool_name} args")),
                                ]),
                                width,
                            );
                            if !collapsed {
                                for l in pretty.lines().take(80) {
                                    let l = sanitize_tui_text(l);
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(Span::styled(
                                            format!("  {}", l),
                                            Style::default().add_modifier(Modifier::DIM),
                                        )),
                                        width,
                                    );
                                }
                            }
                        }
                    }

                    let result_ty = action_type
                        .get("result")
                        .and_then(|v| v.get("type"))
                        .and_then(|v| v.as_str());
                    let value = action_type.get("result").and_then(|v| v.get("value"));
                    if let (Some(result_ty), Some(value)) = (result_ty, value) {
                        if collapsed {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  ▸ ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        "result (collapsed)".to_string(),
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                ]),
                                width,
                            );
                        } else if result_ty == "markdown" {
                            let md = value.as_str().unwrap_or("");
                            for l in render_markdown(
                                md,
                                width.saturating_sub(2).max(1),
                                MdSoftBreakMode::Space,
                            ) {
                                let mut spans = vec![Span::styled(
                                    "  ",
                                    Style::default().add_modifier(Modifier::DIM),
                                )];
                                spans.extend(l.spans.into_iter());
                                push_line(lines, map, entry_idx, Line::from(spans), width);
                            }
                        } else if result_ty == "json" {
                            if let Ok(pretty) = serde_json::to_string_pretty(value) {
                                for l in pretty.lines().take(200) {
                                    let l = sanitize_tui_text(l);
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(Span::styled(
                                            format!("  {}", l),
                                            Style::default().add_modifier(Modifier::DIM),
                                        )),
                                        width,
                                    );
                                }
                            }
                        }
                    }
                }
                _ => {
                    if !content_text.trim().is_empty() {
                        append_text_block(
                            lines,
                            map,
                            entry_idx,
                            label,
                            accent,
                            content_text,
                            width,
                            render_mode,
                        );
                    }
                }
            }
        }
        other => {
            // Fallback: preserve existing behavior.
            let fallback = other.replace('_', " ");
            if !content_text.trim().is_empty() {
                append_text_block(
                    lines,
                    map,
                    entry_idx,
                    &fallback,
                    Color::Gray,
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

fn normalized_entry_text(entry: &serde_json::Value) -> Option<String> {
    let content = entry
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !content.trim().is_empty() {
        return Some(content);
    }

    let entry_type = entry.get("entry_type")?;
    let ty = entry_type.get("type").and_then(|v| v.as_str())?;
    let label = match ty {
        "tool_use" => {
            let tool = entry_type
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("tool");
            let status = entry_type
                .get("status")
                .and_then(|v| v.get("status"))
                .and_then(|v| v.as_str())
                .unwrap_or("created");
            format!("{tool} ({status})")
        }
        "next_action" => "next action".to_string(),
        "loading" => "loading…".to_string(),
        "thinking" => "thinking…".to_string(),
        other => other.replace('_', " "),
    };

    Some(label)
}
