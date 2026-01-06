use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::{
    layout::compute_main_layout,
    state::{AppState, TaskRow},
    ui::{
        render_board_pane, render_bottom_bar, render_branch_picker_modal,
        render_composer_autocomplete, render_confirm_modal, render_create_task_modal,
        render_diff_pane, render_execution_pane, render_help_modal, render_input_modal,
        render_project_setup_modal, render_top_bar,
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

    if app.ui.show_help {
        render_help_modal(f);
    }

    if let Some(state) = app.ui.create_task.as_ref() {
        render_create_task_modal(f, app, state);
    }

    if let Some(confirm) = app.ui.confirm.as_ref() {
        render_confirm_modal(f, confirm);
    }

    if let Some(input) = app.ui.input.as_ref() {
        render_input_modal(f, input);
    }

    if let Some(state) = app.ui.project_setup.as_ref() {
        render_project_setup_modal(f, state);
    }

    if let Some(state) = app.ui.branch_picker.as_ref() {
        render_branch_picker_modal(f, state);
    }
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
