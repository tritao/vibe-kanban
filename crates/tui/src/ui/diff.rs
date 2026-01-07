mod preview;
mod repo_bar;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
};
pub(crate) use repo_bar::{DiffRepoAction, diff_repo_bar_action_at, trigger_diff_repo_action};

use crate::{
    diff::{DIFF_ALL_KEY, diff_rows_with_all_filtered},
    events::StreamStatus,
    state::{AppState, DiffFocus, FocusPane},
    text::display_width,
    util::window_for_list,
};

pub(crate) fn render_diff_pane(f: &mut Frame, app: &AppState, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Min(3),
        ])
        .split(area);

    repo_bar::render_diff_repo_bar(f, app, sections[0]);
    render_diff_files(f, app, sections[1]);
    preview::render_diff_preview(f, app, sections[2]);
}

fn repo_name_from_path(path: &str) -> Option<&str> {
    let first = path.split('/').next()?;
    if first.is_empty() { None } else { Some(first) }
}

fn selected_repo_status_from_diff(app: &AppState) -> Option<usize> {
    if app.diff.repo_statuses.is_empty() {
        return None;
    }
    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
    let selected = rows.get(app.diff.selected_diff_index)?;
    let path = selected
        .new_path
        .as_deref()
        .or(selected.old_path.as_deref())
        .unwrap_or(&selected.key);
    let repo = repo_name_from_path(path)?;
    app.diff
        .repo_statuses
        .iter()
        .position(|r| r.repo_name == repo)
}

pub(crate) fn sync_selected_repo_from_diff_selection(app: &mut AppState) {
    if let Some(idx) = selected_repo_status_from_diff(app) {
        crate::selection_hooks::set_selected_repo_index(app, idx);
    }
}

fn render_diff_files(f: &mut Frame, app: &AppState, area: Rect) {
    f.render_widget(Clear, area);
    if app.diff.list_mode == crate::state::DiffListMode::Commits {
        return render_commit_list(f, app, area);
    }
    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
    let file_count = rows.len().saturating_sub(1);
    let border_style = if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files {
        Style::default().fg(Color::Cyan)
    } else if app.ui.focus == FocusPane::Diff {
        Style::default()
    } else {
        Style::default()
    };

    let title = format!(
        "Files ({}, {}, untracked:{})",
        file_count,
        match app.diff.diff_status {
            StreamStatus::Connected => "live",
            StreamStatus::Connecting => "connecting",
            StreamStatus::Completed => "done",
            StreamStatus::Disconnected => "offline",
            StreamStatus::Error => "error",
        },
        if app.diff.diff_show_untracked {
            "on"
        } else {
            "off"
        }
    );

    let selected = if rows.is_empty() {
        0
    } else {
        app.diff.selected_diff_index.min(rows.len() - 1)
    };

    let height = area.height.saturating_sub(2) as usize;
    let (start, end, selected_in_window) = window_for_list(rows.len(), selected, height);
    let visible = &rows[start..end];

    let items: Vec<ListItem> = if visible.is_empty() {
        vec![ListItem::new(Line::from("No diffs"))]
    } else {
        visible
            .iter()
            .map(|d| {
                const LABEL_W: usize = 8;

                fn label_spans(label: &str, style: Style) -> Vec<Span<'static>> {
                    const LABEL_W: usize = 8;
                    let pad = LABEL_W.saturating_sub(label.len());
                    vec![
                        Span::raw(" ".repeat(pad)),
                        Span::styled(label.to_string(), style),
                        Span::raw(" "),
                    ]
                }

                if d.key == DIFF_ALL_KEY {
                    let mut spans: Vec<Span<'static>> = vec![];
                    spans.extend(label_spans(
                        "ALL",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(
                        "All changes".to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));

                    if d.content_omitted {
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            "[omitted]".to_string(),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }

                    if let (Some(adds), Some(dels)) = (d.additions, d.deletions) {
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            format!("+{adds}"),
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::raw("/"));
                        spans.push(Span::styled(
                            format!("-{dels}"),
                            Style::default()
                                .fg(Color::White)
                                .bg(Color::Red)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }

                    return ListItem::new(Line::from(spans));
                }

                let change = d.change.as_deref().unwrap_or("unknown");
                let (label, change_style) = match change {
                    "added" | "Added" => ("ADD", Style::default().fg(Color::Green)),
                    "deleted" | "Deleted" => ("DEL", Style::default().fg(Color::Red)),
                    "modified" | "Modified" => ("MOD", Style::default().fg(Color::Yellow)),
                    "renamed" | "Renamed" => ("REN", Style::default().fg(Color::Cyan)),
                    "copied" | "Copied" => ("CPY", Style::default().fg(Color::Blue)),
                    "permission_change" | "PermissionChange" | "Permission Change" => {
                        ("CHMOD", Style::default().fg(Color::Magenta))
                    }
                    _ => ("?", Style::default().add_modifier(Modifier::DIM)),
                };

                let path_display = if label == "REN" {
                    match (d.old_path.as_deref(), d.new_path.as_deref()) {
                        (Some(old), Some(new)) if !old.is_empty() && !new.is_empty() => {
                            format!("{old} → {new}")
                        }
                        _ => d.key.clone(),
                    }
                } else {
                    d.key.clone()
                };

                let (dir_part, base_part) = match path_display.rsplit_once('/') {
                    Some((dir, base)) if !dir.is_empty() => {
                        (Some(dir.to_string()), base.to_string())
                    }
                    _ => (None, path_display),
                };

                let mut spans: Vec<Span<'static>> = vec![];
                let styled_label = if label.len() <= LABEL_W && label != "?" {
                    change_style.add_modifier(Modifier::BOLD)
                } else {
                    change_style
                };
                spans.extend(label_spans(label, styled_label));

                if let Some(dir) = dir_part {
                    spans.push(Span::styled(
                        format!("{dir}/"),
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
                spans.push(Span::styled(
                    base_part,
                    Style::default().add_modifier(Modifier::BOLD),
                ));

                if d.content_omitted {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        "[omitted]",
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }

                if let (Some(a), Some(b)) = (d.additions, d.deletions) {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("+{a}"),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::raw("/"));
                    spans.push(Span::styled(
                        format!("-{b}"),
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if let Some(a) = d.additions {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("+{a}"),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if let Some(b) = d.deletions {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("-{b}"),
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect()
    };

    let widget = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(
            if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files {
                "▶ "
            } else {
                "  "
            },
        );

    let mut state = ratatui::widgets::ListState::default();
    if !visible.is_empty() {
        state.select(Some(selected_in_window));
    }
    f.render_stateful_widget(widget, area, &mut state);
}

fn render_commit_list(f: &mut Frame, app: &AppState, area: Rect) {
    f.render_widget(Clear, area);
    let border_style = if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files {
        Style::default().fg(Color::Cyan)
    } else if app.ui.focus == FocusPane::Diff {
        Style::default()
    } else {
        Style::default()
    };

    let repo_id = app
        .diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .map(|r| r.repo_id);
    let commits = repo_id
        .and_then(|id| app.diff.commits_by_repo.get(&id))
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let loading = repo_id
        .and_then(|id| app.diff.commits_loading_by_repo.get(&id))
        .map(|i| i.visible)
        .unwrap_or(false);
    let has_more = repo_id
        .and_then(|id| app.diff.commits_has_more_by_repo.get(&id).copied())
        .unwrap_or(false);

    let selected = if commits.is_empty() {
        0
    } else {
        app.diff.selected_commit_index.min(commits.len() - 1)
    };

    let title = format!(
        "Commits ({}){}",
        commits.len(),
        if loading { ", loading" } else { "" }
    );

    let height = area.height.saturating_sub(2) as usize;
    let (start, end, selected_in_window) = window_for_list(commits.len(), selected, height);
    let visible = commits.get(start..end).unwrap_or(&[]);

    let mut items: Vec<ListItem> = if visible.is_empty() {
        vec![ListItem::new(Line::from("No commits"))]
    } else {
        visible
            .iter()
            .map(|c| {
                let mut spans: Vec<Span<'static>> = vec![];
                spans.push(Span::styled(
                    c.short_oid.clone(),
                    Style::default().add_modifier(Modifier::DIM),
                ));
                spans.push(Span::raw(" "));
                let content_width = area.width.saturating_sub(2) as usize;
                let hash_w = display_width(&c.short_oid) + 1;
                let avail = content_width.saturating_sub(hash_w).max(1);
                let rendered = crate::logs::markdown::render_markdown(
                    c.subject.as_str(),
                    avail,
                    crate::logs::markdown::MdSoftBreakMode::Space,
                );
                if let Some(first) = rendered.first() {
                    spans.extend(first.spans.iter().cloned());
                    if rendered.len() > 1 {
                        spans.push(Span::styled(
                            " …".to_string(),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }
                } else {
                    spans.push(Span::raw(c.subject.clone()));
                }
                ListItem::new(Line::from(spans))
            })
            .collect()
    };
    if loading {
        items.push(ListItem::new(Line::from(Span::styled(
            "Loading…",
            Style::default().add_modifier(Modifier::DIM),
        ))));
    } else if has_more {
        items.push(ListItem::new(Line::from(Span::styled(
            "PgDn: older commits",
            Style::default().add_modifier(Modifier::DIM),
        ))));
    }

    let widget = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(
            if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files {
                "▶ "
            } else {
                "  "
            },
        );

    let mut state = ratatui::widgets::ListState::default();
    if !visible.is_empty() {
        state.select(Some(selected_in_window));
    }
    f.render_stateful_widget(widget, area, &mut state);
}

// Rendered via `preview::render_diff_preview`.
