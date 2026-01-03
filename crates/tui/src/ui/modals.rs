use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::layout::centered_rect;
use crate::state::{ConfirmState, InputMode, InputState};

pub(crate) fn render_help_modal(f: &mut Frame) {
    let area = centered_rect(70, 70, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(vec![Span::styled(
            "Vibe Kanban TUI — Help",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Global"),
        Line::from("  q           quit"),
        Line::from("  Tab         cycle focus"),
        Line::from("  /           search tasks"),
        Line::from("  r           reconnect streams"),
        Line::from("  ? / Esc     close help"),
        Line::from(""),
        Line::from("Board (left)"),
        Line::from("  n           new task"),
        Line::from("  j/k or ↑/↓  move within status"),
        Line::from("  J/K         change status section"),
        Line::from("  ←/→         move task status"),
        Line::from("  [ / ]       switch attempt"),
        Line::from("  c           toggle cancelled section"),
        Line::from(""),
        Line::from("Execution (center)"),
        Line::from("  i           compose follow-up"),
        Line::from("  Enter       send follow-up (while composing)"),
        Line::from("  Ctrl+Enter  insert newline (while composing)"),
        Line::from("  /<cmd>      run slash command (while composing)"),
        Line::from("  Tab         autocomplete (slash mode)"),
        Line::from("  ↑/↓         move cursor / select suggestion at end"),
        Line::from("  e / Enter   expand/collapse entry"),
        Line::from("  Esc         cancel compose"),
        Line::from("  o           toggle raw/normalized"),
        Line::from("  PgUp/PgDn   scroll logs"),
        Line::from("  End         jump bottom"),
        Line::from("  v           toggle timeline/run"),
        Line::from("  x           stop active run"),
        Line::from(""),
        Line::from("Slash commands"),
        Line::from("  /status                 refresh repo branch status"),
        Line::from("  /repo [name|n]           select repo for git ops"),
        Line::from("  /rebase [--onto B]       rebase attempt branch"),
        Line::from("  /resolve [--repo R]      ask agent to resolve conflicts"),
        Line::from("  /merge                   squash-merge into target"),
        Line::from("  /push [--force]          push branch"),
        Line::from("  /abort                   abort conflicts/rebase"),
        Line::from("  /pr create --title T     create PR (server-side)"),
        Line::from("  /pr attach               attach existing PR"),
        Line::from("  /open <file>             open file in editor"),
        Line::from(""),
        Line::from("Diff (right)"),
        Line::from("  j/k         select file"),
        Line::from("  h/l         files/preview focus"),
        Line::from("  PgUp/PgDn   scroll diff preview"),
        Line::from("  d           toggle stats-only"),
        Line::from("  t           cycle theme"),
        Line::from("  M           merge (selected repo)"),
        Line::from("  P           create PR (selected repo)"),
        Line::from("  U / Enter   open PR (selected repo)"),
        Line::from("  R           rebase (selected repo)"),
        Line::from("  S           refresh branch status"),
        Line::from("  C           resolve conflicts (agent)"),
        Line::from("  O           open first conflicted file"),
        Line::from("  A           abort conflicts/rebase"),
    ];

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

pub(crate) fn render_confirm_modal(f: &mut Frame, confirm: &ConfirmState) {
    let area = centered_rect(70, 35, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(vec![Span::styled(
            confirm.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(confirm.body.clone()),
        Line::from(""),
        Line::from("y = confirm, n/Esc = cancel"),
    ];

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Confirm"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

pub(crate) fn render_input_modal(f: &mut Frame, input: &InputState) {
    let area = centered_rect(80, 25, f.area());
    f.render_widget(Clear, area);

    let (title, hint) = match input.mode {
        InputMode::SearchTasks => (
            "Search tasks",
            "type to filter, Enter to apply, Esc to cancel",
        ),
    };

    let lines = vec![
        Line::from(vec![Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(format!("/{}", input.buffer)),
        Line::from(""),
        Line::from(hint),
    ];

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}
