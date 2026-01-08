use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::ListItem,
};

pub(crate) fn push_loading_or_more_hint(
    items: &mut Vec<ListItem<'static>>,
    loading: bool,
    has_more: bool,
    more_hint: &'static str,
) {
    if loading {
        items.push(ListItem::new(Line::from(Span::styled(
            "Loading…",
            Style::default().add_modifier(Modifier::DIM),
        ))));
        return;
    }
    if has_more {
        items.push(ListItem::new(Line::from(Span::styled(
            more_hint,
            Style::default().add_modifier(Modifier::DIM),
        ))));
    }
}
