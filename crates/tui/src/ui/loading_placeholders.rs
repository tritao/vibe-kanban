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
