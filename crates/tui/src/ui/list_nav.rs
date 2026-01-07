use ratatui::layout::Rect;

use crate::{selection::clamp_index, util::window_for_list};

pub(crate) fn select_index(selected: &mut usize, idx: usize, len: usize) -> bool {
    if len == 0 {
        *selected = 0;
        return false;
    }
    let next = idx.min(len.saturating_sub(1));
    if next == *selected {
        return false;
    }
    *selected = next;
    true
}

pub(crate) fn select_delta(selected: &mut usize, delta: i32, len: usize) -> bool {
    if len == 0 {
        *selected = 0;
        return false;
    }
    let cur = (*selected).min(len.saturating_sub(1));
    let next = clamp_index(cur, delta, len);
    if next == cur {
        return false;
    }
    *selected = next;
    true
}

pub(crate) fn index_at_row(
    area: Rect,
    row: u16,
    selected: usize,
    len: usize,
    footer_rows: usize,
) -> Option<usize> {
    if len == 0 {
        return None;
    }

    // Inside list border.
    let inner_y0 = area.y.saturating_add(1);
    let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
    if row < inner_y0 || row >= inner_y1 {
        return None;
    }

    let height = area.height.saturating_sub(2) as usize;
    let height = height.saturating_sub(footer_rows);
    if height == 0 {
        return None;
    }

    let selected = selected.min(len.saturating_sub(1));
    let (start, end, _) = window_for_list(len, selected, height);
    let visible_len = end.saturating_sub(start);

    let inner_row = row.saturating_sub(inner_y0) as usize;
    if inner_row >= visible_len {
        return None;
    }
    Some(start + inner_row)
}
