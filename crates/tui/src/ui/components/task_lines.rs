use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::selection::BoardTaskItem;

pub(crate) fn render_board_task_line(item: &BoardTaskItem) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![];

    if item.indent > 0 {
        spans.push(Span::styled(
            "  ".repeat(item.indent as usize),
            Style::default().add_modifier(Modifier::DIM),
        ));
        spans.push(Span::styled(
            "↳ ",
            Style::default().add_modifier(Modifier::DIM),
        ));
    }

    if item.task.has_in_progress_attempt {
        spans.push(Span::styled(
            "RUN ",
            Style::default().fg(crate::ui::palette::notice_fg()),
        ));
    } else if item.task.last_attempt_failed {
        spans.push(Span::styled("FAIL", crate::ui::palette::badge_fail()));
        spans.push(Span::raw(" "));
    }

    if item.indent > 0 && item.task.status != item.root_status {
        spans.push(Span::styled(
            format!("({}) ", item.task.status.label()),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }

    spans.push(Span::raw(item.task.title.clone()));
    if let Some(executor) = item.task.executor.as_ref().filter(|s| !s.trim().is_empty()) {
        spans.push(Span::styled(
            format!(" · {executor}"),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }
    Line::from(spans)
}
