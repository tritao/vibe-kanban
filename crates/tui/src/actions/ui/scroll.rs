use crate::{
    layout::{compute_main_layout, current_terminal_rect},
    state::{AppState, FocusPane},
};

fn exec_visible_lines() -> usize {
    let layout = compute_main_layout(current_terminal_rect(), FocusPane::Execution);
    layout.exec_logs.height.saturating_sub(2) as usize
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
