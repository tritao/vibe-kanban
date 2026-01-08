use ratatui::widgets::ListItem;

pub(crate) fn push_loading_or_more_hint(
    items: &mut Vec<ListItem<'static>>,
    loading: bool,
    has_more: bool,
    more_hint: &'static str,
) {
    if loading {
        items.push(ListItem::new(crate::ui::widgets::dim_line("Loading…")));
        return;
    }
    if has_more {
        items.push(ListItem::new(crate::ui::widgets::dim_line(more_hint)));
    }
}
