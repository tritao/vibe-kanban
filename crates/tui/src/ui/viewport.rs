use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Clear, Paragraph},
};

pub(crate) fn pad_lines_to_height(lines: &[Line<'static>], height: usize) -> Vec<Line<'static>> {
    let mut out = lines.to_vec();
    out.resize(height, Line::from(""));
    out
}

pub(crate) fn render_cleared_padded_paragraph(
    f: &mut Frame,
    area: Rect,
    make: impl FnOnce(Vec<Line<'static>>) -> Paragraph<'static>,
    lines: &[Line<'static>],
) {
    f.render_widget(Clear, area);
    let height = area.height.saturating_sub(2) as usize;
    let paragraph = make(pad_lines_to_height(lines, height));
    f.render_widget(paragraph, area);
}
