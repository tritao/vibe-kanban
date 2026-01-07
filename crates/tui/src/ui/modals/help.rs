use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::component::ModalComponent;
use crate::ui::layout::centered_rect;

pub(crate) struct HelpModal;
pub(crate) static MODAL: HelpModal = HelpModal;

impl ModalComponent for HelpModal {
    fn is_open(&self, app: &crate::state::AppState) -> bool {
        app.ui.show_help
    }

    fn render(&self, f: &mut Frame, _app: &crate::state::AppState) {
        render_help_modal(f);
    }

    fn on_key(&self, app: &mut crate::state::AppState, key: crossterm::event::KeyEvent) -> bool {
        handle_help_key(app, key)
    }
}

pub(crate) fn render_help_modal(f: &mut Frame) {
    let area = centered_rect(70, 70, f.area());
    f.render_widget(Clear, area);

    let mut lines = vec![
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
        Line::from("  d           delete task"),
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
        Line::from("  Ctrl+z/y    undo/redo (while composing)"),
        Line::from("  Ctrl+←/→    move by word"),
        Line::from("  Alt+Backsp  delete word"),
        Line::from("  y           copy selected logs/visible"),
        Line::from("  Y           copy worktree path"),
        Line::from("  W           copy attempt checkout path"),
        Line::from("  Alt-s       toggle mouse capture (text selection)"),
        Line::from("  drag        select log text (in-app)"),
        Line::from("  e / Enter   expand/collapse entry"),
        Line::from("  Esc         cancel compose"),
        Line::from("  o           toggle raw/normalized"),
        Line::from("  PgUp/PgDn   scroll logs"),
        Line::from("  End         jump bottom"),
        Line::from("  v           toggle timeline/run"),
        Line::from("  x           stop active run"),
        Line::from(""),
        Line::from("Slash commands"),
    ];
    lines.extend(
        crate::slash::help_section_lines()
            .into_iter()
            .map(Line::from),
    );
    lines.extend([
        Line::from(""),
        Line::from("Diff (right)"),
        Line::from("  j/k         select file"),
        Line::from("  h/l         files/preview focus"),
        Line::from("  PgUp/PgDn   scroll diff preview"),
        Line::from("  y           copy selected path/diff"),
        Line::from("  Y           copy worktree path"),
        Line::from("  W           copy attempt checkout path"),
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
    ]);

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

pub(crate) fn handle_help_key(
    app: &mut crate::state::AppState,
    key: crossterm::event::KeyEvent,
) -> bool {
    match key.code {
        crossterm::event::KeyCode::Char('?') | crossterm::event::KeyCode::Esc => {
            app.ui.show_help = false;
            true
        }
        _ => false,
    }
}
