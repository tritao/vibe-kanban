use ratatui::{style::Style, text::Span};

use crate::text::display_width;

#[derive(Clone)]
pub(crate) struct ButtonSpec<T> {
    pub(crate) id: T,
    pub(crate) label: String,
    pub(crate) style: Style,
    pub(crate) enabled: bool,
}

pub(crate) fn button_row_plain<T>(specs: &[ButtonSpec<T>]) -> String {
    specs
        .iter()
        .map(|b| b.label.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn push_button_row_spans<T: Copy>(
    spans: &mut Vec<Span<'static>>,
    specs: &[ButtonSpec<T>],
) {
    for (idx, b) in specs.iter().enumerate() {
        spans.push(Span::styled(b.label.clone(), b.style));
        if idx + 1 < specs.len() {
            spans.push(Span::raw(" "));
        }
    }
}

pub(crate) fn hit_test_button_row<T: Copy>(
    specs: &[ButtonSpec<T>],
    inner_col: usize,
    start_col: usize,
) -> Option<T> {
    let mut cursor = start_col;
    for (idx, b) in specs.iter().enumerate() {
        let start = cursor;
        let end = start.saturating_add(display_width(&b.label));
        if inner_col >= start && inner_col < end {
            if !b.enabled {
                return None;
            }
            return Some(b.id);
        }
        cursor = end;
        if idx + 1 < specs.len() {
            cursor = cursor.saturating_add(1);
        }
    }
    None
}
