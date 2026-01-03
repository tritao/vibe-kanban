use crate::layout::{compute_main_layout, current_terminal_rect};
use crate::state::AppState;

fn exec_visible_lines() -> usize {
    let layout = compute_main_layout(current_terminal_rect());
    layout.exec_logs.height.saturating_sub(2) as usize
}

fn diff_visible_lines() -> usize {
    let layout = compute_main_layout(current_terminal_rect());
    layout.diff_preview.height.saturating_sub(2) as usize
}

fn clamp_to_max(offset: usize, len: usize, visible: usize) -> usize {
    let max_off = len.saturating_sub(visible.max(1));
    offset.min(max_off)
}

pub(super) fn normalize_exec_scroll(app: &mut AppState) {
    let len = app.exec.log_lines.len();
    let visible = exec_visible_lines();
    app.exec.log_scroll_offset = clamp_to_max(app.exec.log_scroll_offset, len, visible);
    if app.exec.log_scroll_offset == 0 {
        app.exec.log_autoscroll = true;
    }
}

pub(super) fn scroll_exec_older(app: &mut AppState, lines: usize) {
    app.exec.log_autoscroll = false;
    app.exec.log_scroll_offset = app.exec.log_scroll_offset.saturating_add(lines);
    normalize_exec_scroll(app);
}

pub(super) fn scroll_exec_newer(app: &mut AppState, lines: usize) {
    app.exec.log_scroll_offset = app.exec.log_scroll_offset.saturating_sub(lines);
    normalize_exec_scroll(app);
    if app.exec.log_scroll_offset == 0 {
        app.exec.log_autoscroll = true;
    }
}

pub(super) fn scroll_exec_to_end(app: &mut AppState) {
    app.exec.log_autoscroll = true;
    app.exec.log_scroll_offset = 0;
}

pub(super) fn normalize_diff_scroll(app: &mut AppState) {
    let len = app.diff.diff_preview_lines.len();
    let visible = diff_visible_lines();
    app.diff.diff_scroll_offset = clamp_to_max(app.diff.diff_scroll_offset, len, visible);
}

pub(super) fn scroll_diff_up(app: &mut AppState, lines: usize) {
    app.diff.diff_scroll_offset = app.diff.diff_scroll_offset.saturating_sub(lines);
    normalize_diff_scroll(app);
}

pub(super) fn scroll_diff_down(app: &mut AppState, lines: usize) {
    app.diff.diff_scroll_offset = app.diff.diff_scroll_offset.saturating_add(lines);
    normalize_diff_scroll(app);
}
