use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::{
    layout::compute_main_layout,
    selection::BoardTaskItem,
    state::{AppState, TaskRow},
    ui::{
        render_board_pane, render_bottom_bar, render_composer_autocomplete, render_diff_pane,
        render_execution_pane, render_top_bar,
    },
};

mod legacy;

pub(crate) fn render(f: &mut Frame, app: &AppState) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());
    let layout = compute_main_layout(f.area(), app.ui.focus);

    let top = render_top_bar(app);
    f.render_widget(top, root[0]);

    render_board_pane(f, app, layout.board);
    render_execution_pane(f, app, layout.exec);
    render_diff_pane(f, app, layout.diff);

    let bottom = render_bottom_bar(app);
    f.render_widget(bottom, root[2]);

    render_composer_autocomplete(f, app, layout.exec_input);

    crate::ui::modals::render_overlays(f, app);
}

pub(crate) fn render_task_line(task: &TaskRow) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![];
    if task.has_in_progress_attempt {
        spans.push(Span::styled("RUN ", Style::default().fg(Color::Green)));
    } else if task.last_attempt_failed {
        spans.push(Span::styled(
            "FAIL",
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::raw(task.title.clone()));
    if let Some(executor) = task.executor.as_ref().filter(|s| !s.trim().is_empty()) {
        spans.push(Span::styled(
            format!(" · {executor}"),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }
    Line::from(spans)
}

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
        spans.push(Span::styled("RUN ", Style::default().fg(Color::Green)));
    } else if item.task.last_attempt_failed {
        spans.push(Span::styled(
            "FAIL",
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ));
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
