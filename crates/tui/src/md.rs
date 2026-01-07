use ratatui::text::Line;

pub(crate) use crate::logs::markdown::MdSoftBreakMode;

pub(crate) fn render(
    md: &str,
    width: usize,
    softbreak_mode: MdSoftBreakMode,
) -> Vec<Line<'static>> {
    crate::logs::markdown::render_markdown(md, width, softbreak_mode)
}
