use std::collections::HashSet;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::text::{display_width, slice_by_display_cols, wrap_line_wordwise};
use crate::events::StreamStatus;
use crate::selection::clamp_index;
use crate::state::{AppState, FocusPane, RepoBranchStatus};

pub(crate) fn render_execution_pane(f: &mut Frame, app: &AppState, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(7)])
        .split(area);

    render_logs_viewer(f, app, sections[0]);
    render_composer(f, app, sections[1]);
}

fn render_logs_viewer(f: &mut Frame, app: &AppState, area: Rect) {
    let border_style = if app.ui.focus == FocusPane::Execution {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let inner_width = area.width.saturating_sub(2) as usize;
    let err_lines = app.ui.last_error.as_ref().map(|e| {
        e.lines()
            .flat_map(|line| {
                wrap_line_wordwise(
                    &Line::from(vec![Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Red),
                    )]),
                    inner_width,
                )
            })
            .collect::<Vec<_>>()
    });
    let notice_lines = app.ui.last_notice.as_ref().map(|m| {
        m.lines()
            .flat_map(|line| {
                wrap_line_wordwise(
                    &Line::from(vec![Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::Green),
                    )]),
                    inner_width,
                )
            })
            .collect::<Vec<_>>()
    });

    let len = app.exec.log_lines.len();
    let max_render = area.height.saturating_sub(2) as usize;
    let visible = max_render.min(len);
    let mut offset = if app.exec.log_autoscroll { 0 } else { app.exec.log_scroll_offset };
    offset = offset.min(len.saturating_sub(visible));
    let start = len.saturating_sub(visible + offset);
    let end = len.saturating_sub(offset);

    let mut text: Vec<Line<'static>> = app.exec.log_lines.get(start..end).unwrap_or(&[]).to_vec();
    if text.is_empty() {
        text.push(Line::from("No logs"));
    }
    if let Some(lines) = err_lines {
        text.push(Line::from(""));
        text.push(Line::from("Last error:"));
        if lines.is_empty() {
            text.push(Line::from(Span::styled(
                "—",
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            text.extend(lines);
        }
    }
    if let Some(lines) = notice_lines {
        text.push(Line::from(""));
        text.push(Line::from("Last notice:"));
        if lines.is_empty() {
            text.push(Line::from(Span::styled(
                "—",
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            text.extend(lines);
        }
    }

    let title = format!(
        "Run Logs ({}, {}, {}, {})",
        match app.exec.log_status {
            StreamStatus::Connected => "live",
            StreamStatus::Connecting => "connecting",
            StreamStatus::Completed => "done",
            StreamStatus::Disconnected => "offline",
            StreamStatus::Error => "error",
        },
        app.exec.log_mode.label(),
        app.exec.log_render_mode.label(),
        app.exec.log_view_mode.label()
    );
    let w = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );
    f.render_widget(w, area);
}

fn render_composer(f: &mut Frame, app: &AppState, area: Rect) {
    let border_style = if app.ui.focus == FocusPane::Execution {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2) as usize;

    let lines: Vec<Line<'static>> = if app.ui.composer_active {
        use crate::text::edit::line_ranges;

        // Keep 1 cell free so the terminal cursor can sit "after" the last character.
        let inner_h = inner_h.max(1);
        let prefix = "  ";
        let prefix_w = display_width(prefix);
        let content_w = inner_w
            .saturating_sub(prefix_w)
            .saturating_sub(1)
            .max(1);

        let ranges = line_ranges(&app.ui.composer.buffer);
        let total_lines = ranges.len().max(1);

        let (cur_line, cur_col) = app.ui.composer.cursor_line_col();
        let cur_line = cur_line.min(total_lines.saturating_sub(1));

        let start_line = (app.ui.composer.scroll_y as usize).min(total_lines.saturating_sub(1));
        let end_line = (start_line + inner_h).min(total_lines);

        let mut out: Vec<Line<'static>> = Vec::with_capacity(inner_h);
        for (idx, (start, end)) in ranges.iter().enumerate().take(end_line).skip(start_line) {
            let line_str = app.ui.composer.buffer.get(*start..*end).unwrap_or("");

            let prefix = if idx == start_line {
                if start_line > 0 { "… " } else { "> " }
            } else {
                "  "
            };

            let line_w = display_width(line_str);
            let start_col = app.ui.composer.scroll_x as usize;
            let left = start_col > 0;

            let mut right = false;
            let mut take = content_w.saturating_sub(left as usize);
            if idx != cur_line && start_col.saturating_add(take) < line_w {
                right = true;
                take = content_w
                    .saturating_sub(left as usize)
                    .saturating_sub(1);
            }

            let mut visible = String::new();
            if left {
                visible.push('…');
            }
            visible.push_str(&slice_by_display_cols(line_str, start_col, take));
            if right {
                visible.push('…');
            }

            if idx == cur_line {
                let cursor_in_chunk = cur_col.saturating_sub(start_col).min(take);
                let cursor_x_in_visible = (left as usize).saturating_add(cursor_in_chunk);
                let cursor_x = area
                    .x
                    .saturating_add(1)
                    .saturating_add(prefix_w as u16)
                    .saturating_add(cursor_x_in_visible as u16)
                    .min(area.x.saturating_add(area.width).saturating_sub(2));
                let cursor_y = area
                    .y
                    .saturating_add(1)
                    .saturating_add((idx - start_line) as u16);
                f.set_cursor_position((cursor_x, cursor_y));
            }

            out.push(Line::from(format!("{prefix}{visible}")));
        }

        if out.is_empty() {
            out.push(Line::from("> "));
            f.set_cursor_position((area.x.saturating_add(3), area.y.saturating_add(1)));
        }

        out
    } else {
        vec![Line::from("Press i to type a follow-up or /command…")]
    };

    let w = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input")
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(w, area);
}

#[derive(Debug, Clone)]
struct CompletionItem {
    insert: String,
    desc: String,
}

fn composer_is_slash_mode(s: &str) -> bool {
    s.trim_start().starts_with('/')
}

fn split_for_completion(s: &str) -> (Vec<&str>, &str, bool) {
    let trimmed = s.trim_start();
    let Some(rest) = trimmed.strip_prefix('/') else {
        return (vec![], "", true);
    };
    let ends_with_space = rest.chars().last().is_some_and(|c| c.is_whitespace());
    let mut tokens: Vec<&str> = rest.split_whitespace().collect();
    if ends_with_space {
        return (tokens, "", true);
    }
    let current = tokens.pop().unwrap_or("");
    (tokens, current, false)
}

fn composer_completion_items(app: &AppState) -> Vec<CompletionItem> {
    if !app.ui.composer_active || !composer_is_slash_mode(&app.ui.composer.buffer) {
        return vec![];
    }

    const COMMANDS: &[(&str, &str)] = &[
        ("help", "show help"),
        ("status", "refresh branch status"),
        ("repo", "select repo for git ops"),
        ("rebase", "rebase attempt branch"),
        ("resolve", "ask agent to resolve conflicts"),
        ("abort", "abort conflicts/rebase"),
        ("merge", "squash-merge into target"),
        ("push", "push attempt branch"),
        ("pr", "PR actions"),
        ("open", "open file in editor"),
    ];

    const REBASE_FLAGS: &[(&str, &str)] =
        &[("--onto", "new base branch"), ("--old", "old base branch"), ("--repo", "repo name or index")];
    const MERGE_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const PUSH_FLAGS: &[(&str, &str)] = &[("--force", "force push"), ("--repo", "repo name or index")];
    const ABORT_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const RESOLVE_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const PR_SUB: &[(&str, &str)] = &[
        ("create", "create a PR"),
        ("attach", "attach existing PR"),
        ("comments", "fetch PR comments count"),
        ("open", "open PR in browser"),
    ];
    const PR_CREATE_FLAGS: &[(&str, &str)] = &[
        ("--title", "PR title (required)"),
        ("--body", "PR body"),
        ("--base", "target branch"),
        ("--draft", "create as draft"),
        ("--auto-desc", "auto-generate description"),
        ("--repo", "repo name or index"),
    ];
    const PR_ATTACH_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const PR_COMMENTS_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];

    let (tokens, current, ends_with_space) = split_for_completion(&app.ui.composer.buffer);

    let current_lower = current.to_ascii_lowercase();
    let used_flags: HashSet<&str> = tokens
        .iter()
        .copied()
        .filter(|t| t.starts_with("--"))
        .collect();

    let mut out: Vec<CompletionItem> = vec![];

    fn push_flags(
        out: &mut Vec<CompletionItem>,
        flags: &[(&'static str, &'static str)],
        used: &HashSet<&str>,
        current_lower: &str,
        current_is_empty: bool,
    ) {
        for (flag, desc) in flags {
            if used.contains(*flag) {
                continue;
            }
            if current_is_empty || flag.starts_with(current_lower) {
                out.push(CompletionItem {
                    insert: format!("{flag} "),
                    desc: desc.to_string(),
                });
            }
        }
    }

    fn push_repos(
        out: &mut Vec<CompletionItem>,
        repos: &[RepoBranchStatus],
        current_lower: &str,
        current_is_empty: bool,
    ) {
        for (idx, r) in repos.iter().enumerate() {
            let name = r.repo_name.as_str();
            let name_l = name.to_ascii_lowercase();
            if current_is_empty || name_l.starts_with(current_lower) || name_l.contains(current_lower) {
                out.push(CompletionItem {
                    insert: format!("{name} "),
                    desc: "repo".to_string(),
                });
            }
            let n = format!("{}", idx + 1);
            if current_is_empty || n.starts_with(current_lower) {
                out.push(CompletionItem {
                    insert: format!("{n} "),
                    desc: "repo index".to_string(),
                });
            }
        }
    }

    if tokens.is_empty() {
        for (cmd, desc) in COMMANDS {
            if current.is_empty() || cmd.starts_with(&current_lower) {
                out.push(CompletionItem {
                    insert: format!("{cmd} "),
                    desc: (*desc).to_string(),
                });
            }
        }
        return out;
    }

    let cmd = tokens[0];
    if !COMMANDS.iter().any(|(c, _)| *c == cmd) {
        for (c, desc) in COMMANDS {
            if c.starts_with(&cmd.to_ascii_lowercase()) {
                out.push(CompletionItem {
                    insert: format!("{c} "),
                    desc: (*desc).to_string(),
                });
            }
        }
        return out;
    }

    let current_is_empty = current.is_empty();

    match cmd {
        "repo" => {
            push_repos(
                &mut out,
                &app.diff.repo_statuses,
                &current_lower,
                current_is_empty,
            );
        }
        "rebase" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(
                    &mut out,
                    REBASE_FLAGS,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(
                    &mut out,
                    &app.diff.repo_statuses,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        "abort" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(
                    &mut out,
                    ABORT_FLAGS,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(
                    &mut out,
                    &app.diff.repo_statuses,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        "resolve" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(
                    &mut out,
                    RESOLVE_FLAGS,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(
                    &mut out,
                    &app.diff.repo_statuses,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        "merge" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(
                    &mut out,
                    MERGE_FLAGS,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(
                    &mut out,
                    &app.diff.repo_statuses,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        "push" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(
                    &mut out,
                    PUSH_FLAGS,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(
                    &mut out,
                    &app.diff.repo_statuses,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        "pr" => {
            if tokens.len() == 1 {
                for (sub, desc) in PR_SUB {
                    if current_is_empty || sub.starts_with(&current_lower) {
                        out.push(CompletionItem {
                            insert: format!("{sub} "),
                            desc: (*desc).to_string(),
                        });
                    }
                }
            } else if tokens.len() >= 2 {
                let sub = tokens[1];
                match sub {
                    "create" => {
                        if current.starts_with("--") || ends_with_space {
                            push_flags(
                                &mut out,
                                PR_CREATE_FLAGS,
                                &used_flags,
                                &current_lower,
                                current_is_empty,
                            );
                        } else if tokens.last().is_some_and(|t| *t == "--repo") {
                            push_repos(
                                &mut out,
                                &app.diff.repo_statuses,
                                &current_lower,
                                current_is_empty,
                            );
                        }
                    }
                    "attach" => {
                        if current.starts_with("--") || ends_with_space {
                            push_flags(
                                &mut out,
                                PR_ATTACH_FLAGS,
                                &used_flags,
                                &current_lower,
                                current_is_empty,
                            );
                        } else if tokens.last().is_some_and(|t| *t == "--repo") {
                            push_repos(
                                &mut out,
                                &app.diff.repo_statuses,
                                &current_lower,
                                current_is_empty,
                            );
                        }
                    }
                    "comments" => {
                        if current.starts_with("--") || ends_with_space {
                            push_flags(
                                &mut out,
                                PR_COMMENTS_FLAGS,
                                &used_flags,
                                &current_lower,
                                current_is_empty,
                            );
                        } else if tokens.last().is_some_and(|t| *t == "--repo") {
                            push_repos(
                                &mut out,
                                &app.diff.repo_statuses,
                                &current_lower,
                                current_is_empty,
                            );
                        }
                    }
                    _ => {
                        for (sub, desc) in PR_SUB {
                            if sub.starts_with(&sub.to_ascii_lowercase()) {
                                out.push(CompletionItem {
                                    insert: format!("{sub} "),
                                    desc: (*desc).to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        "open" | "status" | "help" => {}
        _ => {}
    }

    out
}

pub(crate) fn move_composer_autocomplete(app: &mut AppState, delta: i32) {
    if !composer_is_slash_mode(&app.ui.composer.buffer) {
        return;
    }
    let items = composer_completion_items(app);
    if items.is_empty() {
        return;
    }
    let len = items.len();
    let cur = app.ui.composer_suggest_index.min(len - 1);
    let next = clamp_index(cur, delta, len);
    app.ui.composer_suggest_index = next;
}

pub(crate) fn apply_composer_autocomplete(app: &mut AppState) -> bool {
    if !composer_is_slash_mode(&app.ui.composer.buffer) {
        return false;
    }

    let items = composer_completion_items(app);
    if items.is_empty() {
        return false;
    }

    let idx = app.ui.composer_suggest_index.min(items.len() - 1);
    let insert = items[idx].insert.as_str();

    let cursor = crate::text::edit::clamp_cursor_to_boundary(
        &app.ui.composer.buffer,
        app.ui.composer.cursor,
    );
    let buf_head = app.ui.composer.buffer.get(..cursor).unwrap_or("");
    let mut token_start = buf_head
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);

    if let Some(slash_pos) = buf_head.find('/') {
        if token_start <= slash_pos {
            token_start = slash_pos + 1;
        }
    }

    app.ui.composer.buffer.replace_range(token_start..cursor, insert);
    app.ui.composer.cursor = token_start + insert.len();
    app.ui.composer.goal_col = None;
    app.ui.composer_suggest_index = 0;
    true
}

pub(crate) fn render_composer_autocomplete(f: &mut Frame, app: &AppState, input_area: Rect) {
    if !app.ui.composer_active || !composer_is_slash_mode(&app.ui.composer.buffer) {
        return;
    }

    let items = composer_completion_items(app);
    if items.is_empty() {
        return;
    }

    let max_items = 6usize;
    let visible = items.len().min(max_items);
    let height = (visible + 2).min(input_area.y as usize);
    if height < 3 {
        return;
    }
    let height_u16 = height as u16;
    let y = input_area.y.saturating_sub(height_u16);
    let area = Rect {
        x: input_area.x,
        y,
        width: input_area.width,
        height: height_u16,
    };

    f.render_widget(Clear, area);

    let start = app
        .ui
        .composer_suggest_index
        .saturating_sub(visible.saturating_sub(1));
    let end = (start + visible).min(items.len());
    let window = &items[start..end];

    let list_items: Vec<ListItem> = window
        .iter()
        .cloned()
        .map(|it| {
            let mut spans: Vec<Span<'static>> =
                vec![Span::styled(it.insert.trim().to_string(), Style::default().add_modifier(Modifier::BOLD))];
            if !it.desc.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    it.desc,
                    Style::default().add_modifier(Modifier::DIM),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut state = ratatui::widgets::ListState::default();
    let selected_in_window = app.ui.composer_suggest_index.saturating_sub(start);
    state.select(Some(selected_in_window.min(list_items.len().saturating_sub(1))));

    let w = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Commands")
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");

    f.render_stateful_widget(w, area, &mut state);
}
