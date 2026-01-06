use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    events::StreamStatus,
    fmt::truncate,
    selection::find_task,
    state::{AppState, FocusPane},
};

pub(crate) fn render_top_bar(app: &AppState) -> Paragraph<'static> {
    fn status_badge(label: &'static str, status: StreamStatus) -> Span<'static> {
        let text = match status {
            StreamStatus::Connecting => format!("{label}:…"),
            StreamStatus::Connected => format!("{label}:ok"),
            StreamStatus::Completed => format!("{label}:done"),
            StreamStatus::Disconnected => format!("{label}:off"),
            StreamStatus::Error => format!("{label}:err"),
        };
        let color = match status {
            StreamStatus::Connecting => Color::Yellow,
            StreamStatus::Connected => Color::Green,
            StreamStatus::Completed => Color::Green,
            StreamStatus::Disconnected => Color::DarkGray,
            StreamStatus::Error => Color::Red,
        };
        Span::styled(text, Style::default().fg(color))
    }

    let focus = match app.ui.focus {
        FocusPane::Board => "board",
        FocusPane::Execution => "exec",
        FocusPane::Diff => "diff",
    };

    let project_name = app
        .board
        .selected_project_id
        .and_then(|id| {
            app.board
                .projects_store
                .get("projects")?
                .get(id.to_string())?
                .get("name")?
                .as_str()
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "(no project)".to_string());

    let task_title = app
        .board
        .selected_task_id
        .and_then(|id| find_task(&app.board.tasks_store, id).map(|t| t.title))
        .unwrap_or_else(|| "—".to_string());

    let attempt_branch = app
        .board
        .selected_attempt_id
        .and_then(|id| app.board.attempts.iter().find(|a| a.id == id))
        .map(|a| a.branch.clone())
        .unwrap_or_else(|| "—".to_string());

    let line = Line::from(vec![
        Span::styled("vk-tui", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(
            truncate(&project_name, 18),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  "),
        Span::styled(truncate(&task_title, 28), Style::default()),
        Span::raw("  "),
        Span::styled(
            truncate(&attempt_branch, 18),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw("  "),
        status_badge("tasks", app.board.tasks_status),
        Span::raw(" "),
        status_badge("exec", app.exec.exec_status),
        Span::raw(" "),
        status_badge("diff", app.diff.diff_status),
        Span::raw(" "),
        status_badge("log", app.exec.log_status),
        Span::raw("  "),
        Span::styled(
            format!("mode:{}", app.exec.log_mode.label()),
            Style::default().fg(Color::Gray),
        ),
        Span::raw(" "),
        Span::styled(
            format!("view:{}", app.exec.log_render_mode.label()),
            Style::default().fg(Color::Gray),
        ),
        Span::raw("  "),
        Span::styled(focus, Style::default().add_modifier(Modifier::DIM)),
    ]);

    Paragraph::new(line)
}

pub(crate) fn render_bottom_bar(app: &AppState) -> Paragraph<'static> {
    let text = match app.ui.focus {
        FocusPane::Board => {
            "Tab next | j/k move | J/K status | ←/→ move | / search | [/] attempts | x stop | o log mode | q quit"
        }
        FocusPane::Execution => {
            "Tab next | i compose (/cmd) | Enter send | e expand | PgUp/PgDn scroll | End bottom | m md view | x stop | o log mode | q quit"
        }
        FocusPane::Diff => {
            "Tab next | j/k file | h/l files/preview | PgUp/PgDn scroll | d stats-only | t theme | w wrap | u untracked | B checkout | T target | M merge | P PR | R rebase | S status | q quit"
        }
    };
    if let Some(toast) = app.ui.toast.as_ref() {
        let line = Line::from(vec![
            Span::styled(toast.message.clone(), Style::default().fg(toast.color)),
            Span::raw("  |  "),
            Span::styled(text, Style::default().add_modifier(Modifier::DIM)),
        ]);
        Paragraph::new(line)
    } else {
        Paragraph::new(Line::from(Span::styled(
            text,
            Style::default().add_modifier(Modifier::DIM),
        )))
    }
}
