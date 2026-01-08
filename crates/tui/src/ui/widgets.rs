use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

pub(crate) fn focused_border(active: bool) -> Style {
    if active {
        crate::ui::palette::border_active()
    } else {
        Style::default()
    }
}

pub(crate) fn title_with_tags(base: impl Into<String>, tags: &[(&str, bool)]) -> String {
    let mut out = base.into();
    for (tag, on) in tags {
        if *on {
            out.push_str(", ");
            out.push_str(tag);
        }
    }
    out
}

pub(crate) fn badge(text: impl Into<String>, fg: Color, bg: Color) -> Span<'static> {
    Span::styled(
        format!(" {} ", text.into()),
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
    )
}

pub(crate) fn dim_line(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        text.into(),
        Style::default().add_modifier(Modifier::DIM),
    ))
}
