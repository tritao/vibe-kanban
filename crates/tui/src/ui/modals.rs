use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::layout::centered_rect;
use crate::{
    state::{BranchPickerState, ConfirmState, InputMode, InputState, ProjectSetupState},
    text::{display_width, slice_by_display_cols},
};

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

    // Single-line editor with horizontal scroll.
    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1); // "/" + free cell
    let start_col = input.field.scroll_x as usize;
    let left = start_col > 0;

    let buf = input.field.buffer.as_str();
    let buf_w = display_width(buf);
    let mut right = false;
    let mut take = content_w.saturating_sub(left as usize);
    if start_col.saturating_add(take) < buf_w {
        right = true;
        take = content_w.saturating_sub(left as usize).saturating_sub(1);
    }

    let mut visible = String::new();
    if left {
        visible.push('…');
    }
    visible.push_str(&slice_by_display_cols(buf, start_col, take));
    if right {
        visible.push('…');
    }

    let lines = vec![
        Line::from(vec![Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(format!("/{visible}")),
        Line::from(""),
        Line::from(hint),
    ];

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);

    // Cursor position: title (0), blank (1), input (2).
    let (_, col) = input.field.cursor_line_col();
    let cursor_in_chunk = col.saturating_sub(start_col).min(take);
    let cursor_x_in_visible = (left as usize).saturating_add(cursor_in_chunk);

    let x = area
        .x
        .saturating_add(1)
        .saturating_add(1) // leading "/"
        .saturating_add(cursor_x_in_visible as u16)
        .min(area.x.saturating_add(area.width).saturating_sub(2));
    let y = area.y.saturating_add(1).saturating_add(2);
    f.set_cursor_position((x, y));
}

pub(crate) fn render_project_setup_modal(f: &mut Frame, state: &ProjectSetupState) {
    let area = centered_rect(70, 35, f.area());
    f.render_widget(Clear, area);

    let repo_line = match state.repo_path.as_deref() {
        Some(p) => format!("Repo: {p}"),
        None => "Repo: (not a git repo)".to_string(),
    };

    let mut lines = vec![
        Line::from(vec![Span::styled(
            if state.has_projects {
                "No matching project for this folder"
            } else {
                "No projects found"
            },
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(repo_line),
        Line::from(""),
        Line::from(format!("Create project: {}", state.suggested_project_name)),
        Line::from(""),
    ];

    if state.busy {
        lines.push(Line::from("Creating project…"));
    } else if state.repo_path.is_some() {
        if state.has_projects {
            lines.push(Line::from(
                "Enter = create project, A = add repo to selected project, Esc = dismiss",
            ));
        } else {
            lines.push(Line::from("Enter = create project, Esc = dismiss"));
        }
    } else {
        lines.push(Line::from("Cd into a git repo to create a project."));
        lines.push(Line::from("Esc = dismiss"));
    }

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Project Setup"),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

pub(crate) fn render_branch_picker_modal(f: &mut Frame, state: &BranchPickerState) {
    let area = centered_rect(80, 70, f.area());
    f.render_widget(Clear, area);

    let filter = state.filter.buffer.trim();
    let filter_line = if filter.is_empty() {
        "Filter: (type to search)".to_string()
    } else {
        format!("Filter: {filter}")
    };

    let needle = filter.to_ascii_lowercase();
    let visible: Vec<&crate::state::GitBranchItem> = state
        .branches
        .iter()
        .filter(|b| needle.is_empty() || b.name.to_ascii_lowercase().contains(&needle))
        .collect();

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(vec![Span::styled(
            format!(
                "{} — {}",
                match state.mode {
                    crate::state::BranchPickerMode::Checkout => "Checkout branch",
                    crate::state::BranchPickerMode::ChangeTarget => "Switch target branch",
                },
                state.repo_name
            ),
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(filter_line),
        Line::from(""),
    ];

    if state.busy {
        lines.push(Line::from(Span::styled(
            "Loading…",
            Style::default().add_modifier(Modifier::DIM),
        )));
    } else if let Some(err) = state.error.as_deref() {
        lines.push(Line::from(Span::styled(
            err.to_string(),
            Style::default().fg(ratatui::style::Color::Red),
        )));
    } else if visible.is_empty() {
        lines.push(Line::from(Span::styled(
            "No matching branches.",
            Style::default().add_modifier(Modifier::DIM),
        )));
    } else {
        let max = (area.height as usize).saturating_sub(8).max(6);
        let sel = state.selected_index.min(visible.len().saturating_sub(1));
        let start = sel
            .saturating_sub(max / 2)
            .min(visible.len().saturating_sub(1));
        let end = (start + max).min(visible.len());

        for (i, b) in visible[start..end].iter().enumerate() {
            let absolute = start + i;
            let mut prefix = String::new();
            if b.is_current {
                prefix.push('*');
            } else {
                prefix.push(' ');
            }
            prefix.push(' ');
            if b.is_remote {
                prefix.push('r');
            } else {
                prefix.push(' ');
            }
            prefix.push(' ');

            let style = if absolute == sel {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{prefix}{}", b.name),
                style,
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            match state.mode {
                crate::state::BranchPickerMode::Checkout => "Enter = checkout, Esc = cancel",
                crate::state::BranchPickerMode::ChangeTarget => "Enter = set target, Esc = cancel",
            },
            Style::default().add_modifier(Modifier::DIM),
        )));
    }

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Branches"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}
