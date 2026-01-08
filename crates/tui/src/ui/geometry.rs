use ratatui::layout::Rect;

#[allow(dead_code)]
pub(crate) fn inner_rect(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

pub(crate) fn inner_size(area: Rect) -> (usize, usize) {
    (
        area.width.saturating_sub(2) as usize,
        area.height.saturating_sub(2) as usize,
    )
}
