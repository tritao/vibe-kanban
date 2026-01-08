use std::time::Instant;

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::state::LoadingState;

fn dim_line(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        text.into(),
        Style::default().add_modifier(Modifier::DIM),
    ))
}

pub(crate) fn tick_with_placeholder(
    now: Instant,
    state: &mut LoadingState,
    is_running: bool,
    lines: &mut Vec<Line<'static>>,
    placeholder_text: &'static str,
    clear_pending_once_visible: bool,
    should_insert: impl FnOnce(&[Line<'static>]) -> bool,
) -> bool {
    let mut dirty = false;
    if state.tick(now, is_running) {
        dirty = true;
    }
    if state.visible() && state.placeholder_pending {
        let insert = should_insert(lines);
        if insert {
            *lines = vec![dim_line(placeholder_text)];
        }
        if insert || clear_pending_once_visible {
            state.placeholder_pending = false;
            dirty = true;
        }
    }
    dirty
}

pub(crate) struct NoticePlaceholder {
    pub(crate) dirty: bool,
    pub(crate) show_notice: bool,
}

pub(crate) fn tick_notice_placeholder(
    now: Instant,
    state: &mut LoadingState,
    is_running: bool,
    should_show: bool,
) -> NoticePlaceholder {
    let mut dirty = false;
    if state.pending && !is_running {
        state.stop();
        dirty = true;
    }
    if state.tick(now, is_running) {
        dirty = true;
    }
    let show_notice = state.visible() && state.placeholder_pending && should_show;
    if show_notice {
        state.placeholder_pending = false;
        dirty = true;
    }
    NoticePlaceholder { dirty, show_notice }
}
