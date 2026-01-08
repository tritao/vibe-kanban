use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::UiComponent;
use crate::{
    events::StreamStatus,
    layout::{current_terminal_rect, rect_contains},
    logs::LogSelection,
    state::{AppState, FocusPane},
    text::wrap_line_wordwise,
};

#[derive(Debug, Clone, Copy)]
pub(super) enum ExecLogHitKind {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub(super) struct ExecLogHit {
    pub(super) selection: Option<LogSelection>,
    pub(super) line_idx: Option<usize>,
    pub(super) click_chevron: bool,
}

#[derive(Debug, Clone)]
pub(super) enum ExecLogEvent {
    Key(KeyEvent),
    WheelDelta(i32),
    Hit(ExecLogHitKind, ExecLogHit),
    DragTo(usize),
    DragEnd,
}

pub(super) struct ExecLog;

impl ExecLog {
    fn border_style(app: &AppState) -> Style {
        if app.ui.focus == FocusPane::Execution {
            crate::ui::palette::border_active()
        } else {
            Style::default()
        }
    }

    fn exec_visible_lines() -> usize {
        let layout =
            crate::layout::compute_main_layout(current_terminal_rect(), FocusPane::Execution);
        layout.exec_logs.height.saturating_sub(2) as usize
    }

    fn scroll_older(app: &mut AppState, lines: usize) {
        let len = app.exec.log_lines.len();
        let visible = Self::exec_visible_lines();
        let mut scroll = app.exec.log_scroll;
        scroll.scroll_older(len, visible, lines);
        app.exec.log_scroll = scroll;
    }

    fn scroll_newer(app: &mut AppState, lines: usize) {
        let len = app.exec.log_lines.len();
        let visible = Self::exec_visible_lines();
        let mut scroll = app.exec.log_scroll;
        scroll.scroll_newer(len, visible, lines);
        app.exec.log_scroll = scroll;
    }

    fn scroll_to_end(app: &mut AppState) {
        let mut scroll = app.exec.log_scroll;
        scroll.scroll_to_end();
        app.exec.log_scroll = scroll;
    }

    fn start_index_for_visible(app: &AppState, visible: usize) -> usize {
        let len = app.exec.log_lines.len();
        if len == 0 || visible == 0 {
            return 0;
        }

        let visible = visible.min(len);
        let (start, _) = app.exec.log_scroll.visible_range(len, visible);
        start
    }

    fn hit_line_index(app: &AppState, area: Rect, row: u16) -> Option<usize> {
        if app.exec.log_lines.is_empty() {
            return None;
        }

        let inner_y0 = area.y.saturating_add(1);
        let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
        if row < inner_y0 || row >= inner_y1 {
            return None;
        }

        let visible = area.height.saturating_sub(2) as usize;
        if visible == 0 {
            return None;
        }

        let start = Self::start_index_for_visible(app, visible);
        let inner_row = row.saturating_sub(inner_y0) as usize;
        Some(start.saturating_add(inner_row))
    }

    fn hit_log_selection(app: &AppState, area: Rect, col: u16, row: u16) -> Option<LogSelection> {
        if !rect_contains(area, col, row) {
            return None;
        }

        let line_idx = Self::hit_line_index(app, area, row)?;
        app.exec.log_line_targets.get(line_idx).and_then(|v| *v)
    }

    fn click_has_chevron(app: &AppState, line_idx: usize) -> bool {
        let Some(line) = app.exec.log_lines.get(line_idx) else {
            return false;
        };
        line.spans
            .iter()
            .any(|s| s.content.contains('▸') || s.content.contains('▾'))
    }
}

impl UiComponent for ExecLog {
    type Event = ExecLogEvent;

    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        let inner_width = area.width.saturating_sub(2) as usize;

        let err_lines = app.ui.last_error.as_ref().map(|e| {
            e.text
                .lines()
                .flat_map(|line| {
                    wrap_line_wordwise(
                        &Line::from(vec![Span::styled(
                            line.to_string(),
                            Style::default().fg(crate::ui::palette::error_fg()),
                        )]),
                        inner_width,
                    )
                })
                .collect::<Vec<_>>()
        });
        let notice_lines = app.ui.last_notice.as_ref().map(|m| {
            m.text
                .lines()
                .flat_map(|line| {
                    wrap_line_wordwise(
                        &Line::from(vec![Span::styled(
                            line.to_string(),
                            Style::default().fg(crate::ui::palette::notice_fg()),
                        )]),
                        inner_width,
                    )
                })
                .collect::<Vec<_>>()
        });

        let len = app.exec.log_lines.len();
        let max_render = area.height.saturating_sub(2) as usize;
        let visible = max_render.min(len);

        let (start, end) = app.exec.log_scroll.visible_range(len, visible);

        let mut text: Vec<Line<'static>> = app
            .exec
            .log_lines
            .get(start..end)
            .unwrap_or(&[])
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let absolute = start.saturating_add(i);
                let selected = app
                    .exec
                    .log_mouse_select_range
                    .is_some_and(|(a, b)| absolute >= a && absolute <= b);
                if !selected {
                    return line.clone();
                }
                let spans = line
                    .spans
                    .iter()
                    .map(|s| {
                        Span::styled(s.content.clone(), s.style.add_modifier(Modifier::REVERSED))
                    })
                    .collect::<Vec<_>>();
                Line::from(spans)
            })
            .collect();

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
                .border_style(Self::border_style(app)),
        );
        f.render_widget(w, area);
    }

    fn hit_test(app: &AppState, area: Rect, col: u16, row: u16) -> Option<Self::Event> {
        if !rect_contains(area, col, row) {
            return None;
        }

        let selection = Self::hit_log_selection(app, area, col, row);
        let line_idx = Self::hit_line_index(app, area, row);

        let inner_x0 = area.x.saturating_add(1);
        let click_x = col.saturating_sub(inner_x0) as usize;
        let click_chevron = selection.is_some()
            && click_x <= 3
            && line_idx.is_some_and(|idx| Self::click_has_chevron(app, idx));

        Some(ExecLogEvent::Hit(
            ExecLogHitKind::Left,
            ExecLogHit {
                selection,
                line_idx,
                click_chevron,
            },
        ))
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        if app.ui.focus != FocusPane::Execution {
            return false;
        }

        match event {
            ExecLogEvent::Key(key) => match key.code {
                KeyCode::Char('e') | KeyCode::Enter => {
                    crate::logs::toggle_selected_log_entry(app);
                    true
                }
                KeyCode::PageUp => {
                    Self::scroll_older(app, 40);
                    true
                }
                KeyCode::PageDown => {
                    Self::scroll_newer(app, 40);
                    true
                }
                KeyCode::End => {
                    Self::scroll_to_end(app);
                    true
                }
                _ => false,
            },
            ExecLogEvent::WheelDelta(delta) => {
                let delta = delta.clamp(-200, 200);
                if delta == 0 {
                    return false;
                }
                if delta < 0 {
                    Self::scroll_older(app, (-delta) as usize);
                } else {
                    Self::scroll_newer(app, delta as usize);
                }
                true
            }
            ExecLogEvent::Hit(kind, hit) => match kind {
                ExecLogHitKind::Right => {
                    app.exec.log_selected = hit.selection;
                    crate::logs::toggle_selected_log_entry(app);
                    true
                }
                ExecLogHitKind::Left => {
                    if hit.selection.is_none() {
                        return app.exec.clear_log_selection();
                    }

                    app.exec.log_selected = hit.selection;

                    if let Some(line_idx) = hit.line_idx {
                        app.exec.start_log_mouse_selection(line_idx);
                    }

                    if hit.click_chevron {
                        crate::logs::toggle_selected_log_entry(app);
                    }
                    true
                }
            },
            ExecLogEvent::DragTo(cur) => app.exec.update_log_mouse_selection(cur),
            ExecLogEvent::DragEnd => app.exec.finish_log_mouse_selection(),
        }
    }
}
