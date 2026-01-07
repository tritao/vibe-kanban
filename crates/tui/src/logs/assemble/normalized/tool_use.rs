use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use super::{super::push_line, append_text_block};
use crate::{
    diff::highlight_unified_diff,
    logs::{
        buffer::LogAssemblerState,
        markdown::{MdSoftBreakMode, render_markdown},
        types::{ToolStatus, ToolUseAction},
    },
    state::{DiffTheme, LogRenderMode},
    store::logs::EntryTypeRef,
    text::sanitize_tui_text,
};

fn tool_status(entry_type: &EntryTypeRef<'_>) -> ToolStatus {
    let s = entry_type.tool_status_str().unwrap_or("created");
    ToolStatus::parse(s)
}

pub(super) fn append_tool_use(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    state: &mut LogAssemblerState,
    entry_idx: usize,
    entry_type_ref: &EntryTypeRef<'_>,
    entry_type: &serde_json::Value,
    content_text: &str,
    width: usize,
    render_mode: LogRenderMode,
    diff_theme: DiffTheme,
    collapsed: bool,
) {
    let status = tool_status(entry_type_ref);
    let (status_badge, _status_style) = crate::ui::palette::log_tool_status_badge(status);

    let action_type = entry_type.get("action_type");
    let action_type = match action_type {
        Some(v) => v,
        None => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "Tool",
                crate::ui::palette::log_tool_kind("tool").1,
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

    let action_kind = ToolUseAction::parse(action);
    let (label, accent) = crate::ui::palette::log_tool_kind(action);

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
    match action_kind {
        ToolUseAction::CommandRun => {
            let cmd = action_type
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("command");
            header_spans.push(Span::styled(
                format!("{label} {cmd}"),
                Style::default().add_modifier(Modifier::BOLD),
            ));
        }
        ToolUseAction::FileEdit => {
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

    match action_kind {
        ToolUseAction::FileRead => {
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
        ToolUseAction::Search => {
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
        ToolUseAction::CommandRun => {
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
                            Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                            Span::styled(
                                format!("exit {code}"),
                                Style::default().fg(crate::ui::palette::log_accent_error()),
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
                            Span::styled("  ▸ ", Style::default().add_modifier(Modifier::DIM)),
                            Span::styled(
                                "output (collapsed)".to_string(),
                                Style::default().add_modifier(Modifier::DIM),
                            ),
                        ]),
                        width,
                    );
                } else {
                    for (i, l) in output
                        .lines()
                        .take(crate::logs::constants::MAX_COMMAND_OUTPUT_LINES)
                        .enumerate()
                    {
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
                        if i + 1 == crate::logs::constants::MAX_COMMAND_OUTPUT_LINES {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(Span::styled(
                                    "  … (truncated; press o for Raw logs)".to_string(),
                                    Style::default().add_modifier(Modifier::DIM),
                                )),
                                width,
                            );
                        }
                    }
                }
            }
        }
        ToolUseAction::WebFetch => {
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
        ToolUseAction::FileEdit => {
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
                    let action = c.get("action").and_then(|v| v.as_str()).unwrap_or("change");
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
                                Span::styled("  ▸ ", Style::default().add_modifier(Modifier::DIM)),
                                Span::styled(
                                    "diff (collapsed)".to_string(),
                                    Style::default().add_modifier(Modifier::DIM),
                                ),
                            ]),
                            width,
                        );
                    } else {
                        for c in &changes {
                            if c.get("action").and_then(|v| v.as_str()) != Some("edit") {
                                continue;
                            }
                            let diff = c.get("unified_diff").and_then(|v| v.as_str()).unwrap_or("");
                            if diff.trim().is_empty() {
                                continue;
                            }
                            let body_width = width.saturating_sub(2).max(1);
                            let mut rendered =
                                highlight_unified_diff(path, diff, body_width, diff_theme, false);
                            if rendered.len() > crate::logs::constants::MAX_FILE_EDIT_DIFF_LINES {
                                rendered.truncate(crate::logs::constants::MAX_FILE_EDIT_DIFF_LINES);
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
        ToolUseAction::TaskCreate => {
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
                            Span::styled("  ▸ ", Style::default().add_modifier(Modifier::DIM)),
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
        ToolUseAction::PlanPresentation => {
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
                            Span::styled("  ▸ ", Style::default().add_modifier(Modifier::DIM)),
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
        ToolUseAction::TodoManagement => {
            let todos = action_type.get("todos").and_then(|v| v.as_array()).cloned();
            if let Some(todos) = todos {
                // De-duplicate repeated identical todo lists. Some executors emit the
                // same list multiple times (often separated by thinking/progress).
                let mut fingerprint = String::new();
                for t in &todos {
                    let content = t.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    let status = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
                    fingerprint.push_str(status);
                    fingerprint.push('\t');
                    fingerprint.push_str(content);
                    fingerprint.push('\n');
                }
                let (h, len) = super::fnv1a64(&fingerprint);
                if state.has_last_todos && state.last_todos_hash == h && state.last_todos_len == len
                {
                    return;
                }
                state.has_last_todos = true;
                state.last_todos_hash = h;
                state.last_todos_len = len;

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
                    for t in todos.iter().take(crate::logs::constants::MAX_TODO_ITEMS) {
                        let content = t.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        let status = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
                        let mark = if status == "done" { "[x]" } else { "[ ]" };
                        push_line(
                            lines,
                            map,
                            entry_idx,
                            Line::from(vec![
                                Span::styled("  ", Style::default().add_modifier(Modifier::DIM)),
                                Span::raw(format!("{mark} {content}")),
                            ]),
                            width,
                        );
                    }
                }
            }
        }
        ToolUseAction::Tool => {
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
                            Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                            Span::raw(format!("{tool_name} args")),
                        ]),
                        width,
                    );
                    if !collapsed {
                        for l in pretty
                            .lines()
                            .take(crate::logs::constants::MAX_TOOL_ARGS_LINES)
                        {
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
                            Span::styled("  ▸ ", Style::default().add_modifier(Modifier::DIM)),
                            Span::styled(
                                "result (collapsed)".to_string(),
                                Style::default().add_modifier(Modifier::DIM),
                            ),
                        ]),
                        width,
                    );
                } else if result_ty == "markdown" {
                    let md = value.as_str().unwrap_or("");
                    for l in
                        render_markdown(md, width.saturating_sub(2).max(1), MdSoftBreakMode::Space)
                    {
                        let mut spans = vec![Span::styled(
                            "  ",
                            Style::default().add_modifier(Modifier::DIM),
                        )];
                        spans.extend(l.spans.into_iter());
                        push_line(lines, map, entry_idx, Line::from(spans), width);
                    }
                } else if result_ty == "json" {
                    if let Ok(pretty) = serde_json::to_string_pretty(value) {
                        for l in pretty
                            .lines()
                            .take(crate::logs::constants::MAX_TOOL_JSON_LINES)
                        {
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
        ToolUseAction::Other => {
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
