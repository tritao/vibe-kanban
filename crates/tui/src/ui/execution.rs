use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::{
    events::StreamStatus,
    state::{AppState, FocusPane},
    text::{display_width, slice_by_display_cols, wrap_line_wordwise},
};

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
    let mut offset = if app.exec.log_autoscroll {
        0
    } else {
        app.exec.log_scroll_offset
    };
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
        let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);

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
                take = content_w.saturating_sub(left as usize).saturating_sub(1);
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

pub(crate) fn render_composer_autocomplete(f: &mut Frame, app: &AppState, input_area: Rect) {
    if !app.ui.composer_active || !crate::slash::composer_is_slash_mode(&app.ui.composer.buffer) {
        return;
    }

    let items = crate::slash::composer_completion_items(app);
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
            let mut spans: Vec<Span<'static>> = vec![Span::styled(
                it.insert.trim().to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            )];
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
    state.select(Some(
        selected_in_window.min(list_items.len().saturating_sub(1)),
    ));

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
