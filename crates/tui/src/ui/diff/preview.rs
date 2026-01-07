use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::state::{AppState, DiffFocus, FocusPane};

pub(super) fn render_diff_preview(f: &mut Frame, app: &AppState, area: Rect) {
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
