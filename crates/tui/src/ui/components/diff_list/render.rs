use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::{
    diff::{DIFF_ALL_KEY, diff_rows_with_all_filtered},
    events::StreamStatus,
    state::{AppState, DiffFocus, FocusPane},
    text::display_width,
    util::window_for_list,
};

fn files_border_style(app: &AppState) -> Style {
    if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files {
        crate::ui::palette::border_active()
    } else {
        Style::default()
    }
}

pub(super) fn render_files_list(f: &mut Frame, app: &AppState, area: Rect) {
    let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
    let file_count = rows.len().saturating_sub(1);
    let border_style = files_border_style(app);

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
                    spans.extend(label_spans("ALL", crate::ui::palette::diff_label_all()));
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
                            crate::ui::palette::diff_count_added(),
                        ));
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            format!("-{dels}"),
                            crate::ui::palette::diff_count_deleted(),
                        ));
                    }

                    return ListItem::new(Line::from(spans));
                }

                let label = match (d.old_path.as_deref(), d.new_path.as_deref()) {
                    (Some(_), Some(_)) if d.old_path != d.new_path => "REN",
                    (Some(_), None) => "DEL",
                    (None, Some(_)) => "ADD",
                    _ => "MOD",
                };

                let label_style = match label {
                    "ADD" => crate::ui::palette::diff_label_add(),
                    "DEL" => crate::ui::palette::diff_label_del(),
                    "REN" => crate::ui::palette::diff_label_ren(),
                    "MOD" => crate::ui::palette::diff_label_mod(),
                    _ => Style::default().add_modifier(Modifier::BOLD),
                };

                let mut spans: Vec<Span<'static>> = vec![];
                spans.extend(label_spans(label, label_style));

                let path = d
                    .new_path
                    .as_deref()
                    .or(d.old_path.as_deref())
                    .unwrap_or(&d.key);
                spans.push(Span::raw(path.to_string()));

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
                        crate::ui::palette::diff_count_added(),
                    ));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("-{dels}"),
                        crate::ui::palette::diff_count_deleted(),
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

pub(super) fn render_commit_list(f: &mut Frame, app: &AppState, area: Rect) {
    let border_style = files_border_style(app);

    let repo_id = app
        .diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .map(|r| r.repo_id);
    let commits = repo_id
        .and_then(|id| app.diff.commits_by_repo.get(&id))
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let footer = super::commit_footer(app);
    let loading = footer.loading;
    let has_more = footer.has_more;

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
