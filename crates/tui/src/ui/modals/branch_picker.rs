use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::component::ModalComponent;
use crate::{
    state::{AppState, BranchPickerMode, BranchPickerState},
    ui::layout::centered_rect,
};

pub(crate) struct BranchPickerModal;
pub(crate) static MODAL: BranchPickerModal = BranchPickerModal;

impl ModalComponent for BranchPickerModal {
    fn is_open(&self, app: &AppState) -> bool {
        app.ui.branch_picker.is_some()
    }

    fn render(&self, f: &mut Frame, app: &AppState) {
        if let Some(state) = app.ui.branch_picker.as_ref() {
            render_branch_picker_modal(f, state);
        }
    }

    fn on_key(&self, app: &mut AppState, key: KeyEvent) -> bool {
        handle_branch_picker_key(app, key)
    }
}

pub(crate) fn open_branch_picker(
    app: &mut AppState,
    mode: BranchPickerMode,
    repo_id: uuid::Uuid,
    repo_name: String,
) {
    app.ui.branch_picker = Some(BranchPickerState {
        mode,
        repo_id,
        repo_name: repo_name.clone(),
        filter: Default::default(),
        selected_index: 0,
        branches: vec![],
        busy: true,
        error: None,
    });

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match crate::net::ops::repo_branches_http(&base_url, repo_id).await {
            Ok(branches) => {
                let _ = net_tx
                    .send(crate::events::NetEvent::RepoBranchesLoaded { repo_id, branches })
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(crate::events::NetEvent::RepoBranchesFailed {
                        repo_id,
                        message: format!("failed to load branches: {e}"),
                    })
                    .await;
            }
        }
    });
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
                    BranchPickerMode::Checkout => "Checkout branch",
                    BranchPickerMode::ChangeTarget => "Switch target branch",
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
            Style::default().fg(crate::ui::palette::error_fg()),
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
                BranchPickerMode::Checkout => "Enter = checkout, Esc = cancel",
                BranchPickerMode::ChangeTarget => "Enter = set target, Esc = cancel",
            },
            Style::default().add_modifier(Modifier::DIM),
        )));
    }

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Branches"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

pub(crate) fn handle_branch_picker_key(app: &mut AppState, key: KeyEvent) -> bool {
    let Some(state) = app.ui.branch_picker.as_mut() else {
        return false;
    };

    match key.code {
        KeyCode::Esc => {
            app.ui.branch_picker = None;
            return true;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.selected_index = state.selected_index.saturating_sub(1);
            return true;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.selected_index = state.selected_index.saturating_add(1);
            return true;
        }
        KeyCode::Enter => {
            if state.busy {
                return false;
            }
            let attempt_id = match app.board.selected_attempt_id {
                Some(id) => id,
                None => {
                    app.ui
                        .set_error(crate::ui::messages::errors::BRANCH_PICKER_NO_ATTEMPT_SELECTED);
                    return true;
                }
            };

            let filter = state.filter.buffer.trim().to_ascii_lowercase();
            let visible: Vec<&crate::state::GitBranchItem> = state
                .branches
                .iter()
                .filter(|b| {
                    if filter.is_empty() {
                        true
                    } else {
                        b.name.to_ascii_lowercase().contains(&filter)
                    }
                })
                .collect();

            if visible.is_empty() {
                app.ui
                    .set_error(crate::ui::messages::errors::BRANCH_PICKER_NO_MATCHING_BRANCHES);
                return true;
            }
            let idx = state.selected_index.min(visible.len().saturating_sub(1));
            let branch = visible[idx].name.clone();
            let repo_name = state.repo_name.clone();
            let repo_id = state.repo_id;
            let mode = state.mode;
            state.busy = true;

            let base_url = app.backend_url.clone();
            let net_tx = app.net_tx.clone();
            tokio::spawn(async move {
                let result = match mode {
                    BranchPickerMode::Checkout => {
                        crate::net::ops::checkout_attempt_branch_http(
                            &base_url, attempt_id, &branch,
                        )
                        .await
                    }
                    BranchPickerMode::ChangeTarget => {
                        crate::net::ops::change_target_branch_http(
                            &base_url, attempt_id, repo_id, &branch,
                        )
                        .await
                    }
                };
                match result {
                    Ok(()) => {
                        let _ = net_tx
                            .send(crate::events::NetEvent::Notice(match mode {
                                BranchPickerMode::Checkout => {
                                    format!("Checked out {branch} ({repo_name}).")
                                }
                                BranchPickerMode::ChangeTarget => {
                                    format!("Target branch set to {branch} ({repo_name}).")
                                }
                            }))
                            .await;
                        let _ = net_tx
                            .send(crate::events::NetEvent::GitOpFinished {
                                repo_id: Some(repo_id),
                                kind: crate::events::GitOpKind::Status,
                                ok: true,
                                message: "Git: status updated".to_string(),
                            })
                            .await;
                        if let Ok(statuses) =
                            crate::net::ops::branch_status_http(&base_url, attempt_id).await
                        {
                            let _ = net_tx
                                .send(crate::events::NetEvent::BranchStatusLoaded {
                                    attempt_id,
                                    statuses,
                                })
                                .await;
                        }
                    }
                    Err(e) => {
                        let _ = net_tx
                            .send(crate::events::NetEvent::Error(format!(
                                "{} failed: {e}",
                                match mode {
                                    BranchPickerMode::Checkout => "checkout branch",
                                    BranchPickerMode::ChangeTarget => "change target branch",
                                }
                            )))
                            .await;
                    }
                }
            });

            app.ui.branch_picker = None;
            return true;
        }
        _ => {}
    }

    if !state.busy {
        if crate::text::field_edit::apply_text_field_key(&mut state.filter, key, false) {
            state.selected_index = 0;
            return true;
        }
    }

    false
}
