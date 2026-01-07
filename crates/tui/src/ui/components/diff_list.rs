use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
};

use super::UiComponent;
use crate::{
    diff::{DIFF_ALL_KEY, diff_rows_with_all_filtered},
    events::StreamStatus,
    state::{AppState, DiffFocus, FocusPane},
    text::display_width,
    util::window_for_list,
};

pub(crate) enum DiffListEvent {
    Key(KeyEvent),
    ClickIndex(usize),
    WheelDelta(i32),
}

pub(crate) struct DiffList;

impl DiffList {
    fn select_diff_file(app: &mut AppState, idx: usize) -> bool {
        let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
        if rows.is_empty() {
            app.diff.selected_diff_index = 0;
            return false;
        }
        let next = idx.min(rows.len().saturating_sub(1));
        if next == app.diff.selected_diff_index {
            return false;
        }
        app.diff.selected_diff_index = next;
        crate::selection_hooks::on_diff_file_selected(app);
        true
    }

    fn select_adjacent_diff_file(app: &mut AppState, delta: i32) -> bool {
        let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
        if rows.is_empty() {
            app.diff.selected_diff_index = 0;
            return false;
        }
        let cur = app
            .diff
            .selected_diff_index
            .min(rows.len().saturating_sub(1));
        let next = crate::selection::clamp_index(cur, delta, rows.len());
        if next == cur {
            return false;
        }
        app.diff.selected_diff_index = next;
        crate::selection_hooks::on_diff_file_selected(app);
        true
    }

    fn commits_for_selected_repo<'a>(app: &'a AppState) -> &'a [crate::state::CommitEntry] {
        let repo_id = app
            .diff
            .repo_statuses
            .get(app.diff.selected_repo_index)
            .map(|r| r.repo_id);
        repo_id
            .and_then(|id| app.diff.commits_by_repo.get(&id))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    fn select_commit(app: &mut AppState, idx: usize) -> bool {
        let commits = Self::commits_for_selected_repo(app);
        if commits.is_empty() {
            app.diff.selected_commit_index = 0;
            return false;
        }
        let next = idx.min(commits.len().saturating_sub(1));
        if next == app.diff.selected_commit_index {
            return false;
        }
        app.diff.selected_commit_index = next;
        crate::selection_hooks::on_commit_selected(app);
        true
    }

    fn select_adjacent_commit(app: &mut AppState, delta: i32) -> bool {
        let commits = Self::commits_for_selected_repo(app);
        if commits.is_empty() {
            app.diff.selected_commit_index = 0;
            return false;
        }
        let cur = app
            .diff
            .selected_commit_index
            .min(commits.len().saturating_sub(1));
        let next = crate::selection::clamp_index(cur, delta, commits.len());
        if next == cur {
            return false;
        }
        app.diff.selected_commit_index = next;
        crate::selection_hooks::on_commit_selected(app);
        true
    }

    fn hit_test_diff_file_index(app: &AppState, area: Rect, row: u16) -> Option<usize> {
        let rows = diff_rows_with_all_filtered(&app.diff.diff_store, app.diff.diff_show_untracked);
        if rows.is_empty() {
            return None;
        }

        let inner_y0 = area.y.saturating_add(1);
        let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
        if row < inner_y0 || row >= inner_y1 {
            return None;
        }

        let height = area.height.saturating_sub(2) as usize;
        if height == 0 {
            return None;
        }

        let selected = app
            .diff
            .selected_diff_index
            .min(rows.len().saturating_sub(1));
        let (start, end, _) = window_for_list(rows.len(), selected, height);
        let visible_len = end.saturating_sub(start);

        let inner_row = row.saturating_sub(inner_y0) as usize;
        if inner_row >= visible_len {
            return None;
        }
        Some(start + inner_row)
    }

    fn hit_test_commit_index(app: &AppState, area: Rect, row: u16) -> Option<usize> {
        let commits = Self::commits_for_selected_repo(app);
        if commits.is_empty() {
            return None;
        }

        let inner_y0 = area.y.saturating_add(1);
        let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
        if row < inner_y0 || row >= inner_y1 {
            return None;
        }

        let height = area.height.saturating_sub(2) as usize;
        if height == 0 {
            return None;
        }

        let selected = app
            .diff
            .selected_commit_index
            .min(commits.len().saturating_sub(1));
        let (start, end, _) = window_for_list(commits.len(), selected, height);
        let visible_len = end.saturating_sub(start);

        let inner_row = row.saturating_sub(inner_y0) as usize;
        if inner_row >= visible_len {
            return None;
        }
        Some(start + inner_row)
    }

    pub(crate) fn render(f: &mut Frame, app: &AppState, area: Rect) {
        f.render_widget(Clear, area);
        match app.diff.list_mode {
            crate::state::DiffListMode::Commits => render_commit_list(f, app, area),
            crate::state::DiffListMode::Files => render_files_list(f, app, area),
        }
    }

    pub(crate) fn on_event(app: &mut AppState, event: DiffListEvent) -> bool {
        if app.ui.focus != FocusPane::Diff || app.ui.diff_focus != DiffFocus::Files {
            return false;
        }

        match event {
            DiffListEvent::ClickIndex(idx) => match app.diff.list_mode {
                crate::state::DiffListMode::Files => Self::select_diff_file(app, idx),
                crate::state::DiffListMode::Commits => Self::select_commit(app, idx),
            },
            DiffListEvent::WheelDelta(delta) => {
                let delta = delta.clamp(-50, 50);
                if delta == 0 {
                    return false;
                }
                match app.diff.list_mode {
                    crate::state::DiffListMode::Files => {
                        Self::select_adjacent_diff_file(app, delta)
                    }
                    crate::state::DiffListMode::Commits => Self::select_adjacent_commit(app, delta),
                }
            }
            DiffListEvent::Key(key) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => match app.diff.list_mode {
                    crate::state::DiffListMode::Files => Self::select_adjacent_diff_file(app, -1),
                    crate::state::DiffListMode::Commits => Self::select_adjacent_commit(app, -1),
                },
                KeyCode::Down | KeyCode::Char('j') => match app.diff.list_mode {
                    crate::state::DiffListMode::Files => Self::select_adjacent_diff_file(app, 1),
                    crate::state::DiffListMode::Commits => Self::select_adjacent_commit(app, 1),
                },
                KeyCode::PageDown if app.diff.list_mode == crate::state::DiffListMode::Commits => {
                    crate::commands::request_commit_list_more(app);
                    true
                }
                _ => false,
            },
        }
    }
}

impl UiComponent for DiffList {
    type Event = DiffListEvent;

    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        DiffList::render(f, app, area);
    }

    fn hit_test(app: &AppState, area: Rect, _col: u16, row: u16) -> Option<Self::Event> {
        let idx = match app.diff.list_mode {
            crate::state::DiffListMode::Files => DiffList::hit_test_diff_file_index(app, area, row),
            crate::state::DiffListMode::Commits => DiffList::hit_test_commit_index(app, area, row),
        }?;
        Some(DiffListEvent::ClickIndex(idx))
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        DiffList::on_event(app, event)
    }
}

fn files_border_style(app: &AppState) -> Style {
    if app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Files {
        crate::ui::palette::border_active()
    } else if app.ui.focus == FocusPane::Diff {
        Style::default()
    } else {
        Style::default()
    }
}

fn render_files_list(f: &mut Frame, app: &AppState, area: Rect) {
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

fn render_commit_list(f: &mut Frame, app: &AppState, area: Rect) {
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
