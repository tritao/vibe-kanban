pub(crate) fn clamp_offset(offset: usize, len: usize, visible: usize) -> usize {
    let max_off = len.saturating_sub(visible.max(1));
    offset.min(max_off)
}

pub(crate) fn visible_window_from_top(offset: usize, len: usize, visible: usize) -> (usize, usize) {
    if len == 0 || visible == 0 {
        return (0, 0);
    }
    let visible = visible.min(len);
    let offset = clamp_offset(offset, len, visible);
    let start = offset.min(len);
    let end = (start + visible).min(len);
    (start, end)
}

pub(crate) fn visible_window_from_end(
    offset_from_end: usize,
    len: usize,
    visible: usize,
) -> (usize, usize) {
    if len == 0 || visible == 0 {
        return (0, 0);
    }
    let visible = visible.min(len);
    let offset_from_end = clamp_offset(offset_from_end, len, visible);
    let end = len.saturating_sub(offset_from_end);
    let start = end.saturating_sub(visible);
    (start, end)
}
