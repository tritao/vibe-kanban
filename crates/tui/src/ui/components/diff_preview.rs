use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::UiComponent;
use crate::{
    layout::{compute_main_layout, current_terminal_rect},
    state::{AppState, DiffFocus, FocusPane},
};

pub(crate) enum DiffPreviewEvent {
    Key(KeyEvent),
    WheelDelta(i32),
    Click,
}

pub(crate) struct DiffPreview;

fn clamp_to_max(offset: usize, len: usize, visible: usize) -> usize {
    let max_off = len.saturating_sub(visible.max(1));
    offset.min(max_off)
}

impl DiffPreview {
    pub(crate) fn render(f: &mut Frame, app: &AppState, area: Rect) {
        let border_style = crate::ui::widgets::focused_border(
            app.ui.focus == FocusPane::Diff && app.ui.diff_focus == DiffFocus::Preview,
        );

        let (lines, title) = match app.diff.list_mode {
            crate::state::DiffListMode::Files => (
                &app.diff.diff_preview_lines,
                crate::ui::widgets::title_with_tags(
                    format!("Diff ({})", app.diff.diff_theme.label()),
                    &[
                        ("wrap", app.diff.diff_wrap),
                        ("loading", app.diff.diff_preview_loading.visible()),
                    ],
                ),
            ),
            crate::state::DiffListMode::Commits => (&app.diff.commit_preview_lines, {
                format!(
                    "Commit{}",
                    if app.diff.commit_preview_loading.visible() {
                        " (loading)"
                    } else {
                        ""
                    }
                )
            }),
        };

        let start = app.diff.diff_scroll_offset.min(lines.len());
        let height = area.height.saturating_sub(2) as usize;
        let end = (start + height).min(lines.len());
        let visible = lines.get(start..end).unwrap_or(&[]);

        crate::ui::viewport::render_cleared_padded_paragraph(
            f,
            area,
            |padded| {
                let mut w = Paragraph::new(padded).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .border_style(border_style),
                );
                if app.diff.list_mode == crate::state::DiffListMode::Commits {
                    w = w.wrap(Wrap { trim: false });
                }
                w
            },
            visible,
        );
    }

    fn normalize_scroll(app: &mut AppState) {
        let layout = compute_main_layout(current_terminal_rect(), FocusPane::Diff);
        let visible = layout.diff_preview.height.saturating_sub(2) as usize;
        let len = match app.diff.list_mode {
            crate::state::DiffListMode::Files => app.diff.diff_preview_lines.len(),
            crate::state::DiffListMode::Commits => app.diff.commit_preview_lines.len(),
        };
        app.diff.diff_scroll_offset = clamp_to_max(app.diff.diff_scroll_offset, len, visible);
    }

    fn scroll_up(app: &mut AppState, lines: usize) -> bool {
        let before = app.diff.diff_scroll_offset;
        app.diff.diff_scroll_offset = app.diff.diff_scroll_offset.saturating_sub(lines);
        Self::normalize_scroll(app);
        app.diff.diff_scroll_offset != before
    }

    fn scroll_down(app: &mut AppState, lines: usize) -> bool {
        let before = app.diff.diff_scroll_offset;
        app.diff.diff_scroll_offset = app.diff.diff_scroll_offset.saturating_add(lines);
        Self::normalize_scroll(app);
        app.diff.diff_scroll_offset != before
    }

    pub(crate) fn on_event(app: &mut AppState, event: DiffPreviewEvent) -> bool {
        if app.ui.focus != FocusPane::Diff {
            return false;
        }

        match event {
            DiffPreviewEvent::Click => false,
            DiffPreviewEvent::WheelDelta(delta) => {
                if app.ui.diff_focus != DiffFocus::Preview {
                    return false;
                }
                if delta == 0 {
                    return false;
                }
                if delta < 0 {
                    Self::scroll_up(app, (-delta) as usize)
                } else {
                    Self::scroll_down(app, delta as usize)
                }
            }
            DiffPreviewEvent::Key(key) => match key.code {
                KeyCode::PageUp => Self::scroll_up(app, 20),
                KeyCode::PageDown => {
                    if app.ui.diff_focus == DiffFocus::Files
                        && app.diff.list_mode == crate::state::DiffListMode::Commits
                    {
                        // In commits mode, PgDn is reserved for fetching older commits when the
                        // list has focus.
                        return false;
                    }
                    Self::scroll_down(app, 20)
                }
                _ => false,
            },
        }
    }
}

impl UiComponent for DiffPreview {
    type Event = DiffPreviewEvent;

    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        DiffPreview::render(f, app, area);
    }

    fn hit_test(_app: &AppState, _area: Rect, _col: u16, _row: u16) -> Option<Self::Event> {
        Some(DiffPreviewEvent::Click)
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        DiffPreview::on_event(app, event)
    }
}
