use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::super::buffer::{LogAssemblerState, LogKind};
use crate::text::{line_display_width, sanitize_tui_text};

pub(super) fn append_stream_text(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    state: &mut LogAssemblerState,
    kind: LogKind,
    text: &str,
    style: Style,
    allow_join: bool,
    entry_idx: usize,
    prefix: &str,
    width: usize,
) {
    if text.is_empty() {
        return;
    }

    let ends_with_newline = text.ends_with('\n');
    let mut is_first = true;
    for raw in text.split_terminator('\n') {
        let seg = raw.strip_suffix('\r').unwrap_or(raw);
        let seg = sanitize_tui_text(seg);

        if allow_join
            && is_first
            && state.open
            && state.open_kind == Some(kind)
            && map.last().copied() == Some(entry_idx)
            && lines
                .last()
                .is_some_and(|l| l.spans.len() == 1 && l.spans[0].style == style)
        {
            if let Some(last) = lines.last_mut() {
                if let Some(span) = last.spans.first_mut() {
                    span.content.to_mut().push_str(seg.as_ref());
                }
            }
            // If joining caused the line to overflow, re-wrap it.
            if lines
                .last()
                .is_some_and(|l| line_display_width(l) > width.max(1))
            {
                let line = lines.pop().unwrap();
                let _ = map.pop();
                super::push_line(lines, map, entry_idx, line, width);
            }
        } else {
            super::push_line(
                lines,
                map,
                entry_idx,
                Line::from(Span::styled(format!("{prefix}{}", seg), style)),
                width,
            );
        }

        is_first = false;
    }

    if allow_join && !ends_with_newline {
        state.open = true;
        state.open_kind = Some(kind);
    } else {
        state.open = false;
        state.open_kind = None;
    }
}
