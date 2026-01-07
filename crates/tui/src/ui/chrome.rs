use ratatui::widgets::Paragraph;

use crate::state::AppState;

pub(crate) fn render_top_bar(app: &AppState) -> Paragraph<'static> {
    Paragraph::new(crate::ui::vm::chrome::build_top_bar(app).line)
}

pub(crate) fn render_bottom_bar(app: &AppState) -> Paragraph<'static> {
    Paragraph::new(crate::ui::vm::chrome::build_bottom_bar(app).line)
}
