use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::layout;
use crate::{
    state::{AppState, FocusPane},
    ui::button_row::push_button_row_spans,
};

pub(super) fn render_diff_repo_bar(f: &mut Frame, app: &AppState, area: Rect) {
    let border_style = if app.ui.focus == FocusPane::Diff {
        crate::ui::palette::border_active()
    } else {
        Style::default()
    };

    let w = area.width.saturating_sub(2) as usize;

    let layout::DiffRepoBarLayout {
        left,
        can_show_right,
        badge_plains,
        badge_spans,
        buttons,
        ..
    } = layout::compute_repo_bar_layout(app, w, std::time::Instant::now());

    let mut spans: Vec<Span<'static>> = vec![layout::left_title_span(left)];

    if can_show_right {
        spans.push(Span::raw("  "));
        for (idx, badge) in badge_spans.into_iter().enumerate() {
            if idx > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(badge);
        }
        if !badge_plains.is_empty() {
            spans.push(Span::raw("  "));
        }
        push_button_row_spans(&mut spans, &buttons);
    }

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Repo")
            .border_style(border_style),
    );
    f.render_widget(p, area);
}
