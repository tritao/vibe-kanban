use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    state::{AppState, ProjectSetupState},
    ui::layout::centered_rect,
};

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

pub(crate) fn handle_project_setup_key(app: &mut AppState, key: KeyEvent) -> bool {
    let Some(state) = app.ui.project_setup.as_mut() else {
        return false;
    };
    match key.code {
        KeyCode::Esc => {
            app.ui.project_setup = None;
            app.ui.project_setup_dismissed = true;
            true
        }
        KeyCode::Enter => {
            if state.busy {
                return false;
            }
            let Some(repo_path) = state.repo_path.clone() else {
                app.ui.set_error(
                    "current directory is not a git repository (cd into a repo to create a project)",
                );
                return true;
            };
            state.busy = true;

            let base_url = app.backend_url.clone();
            let net_tx = app.net_tx.clone();
            let name = state.suggested_project_name.clone();
            let display_name = name.clone();

            tokio::spawn(async move {
                match crate::net::ops::create_project_http(
                    &base_url,
                    &name,
                    &repo_path,
                    &display_name,
                )
                .await
                {
                    Ok(project_id) => {
                        let _ = net_tx
                            .send(crate::events::NetEvent::ProjectCreated { project_id })
                            .await;
                        let _ = net_tx
                            .send(crate::events::NetEvent::Notice(format!(
                                "Created project {name}."
                            )))
                            .await;
                    }
                    Err(e) => {
                        let _ = net_tx
                            .send(crate::events::NetEvent::Error(format!(
                                "create project failed: {e}"
                            )))
                            .await;
                    }
                }
            });
            true
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if state.busy {
                return false;
            }
            if !state.has_projects {
                return false;
            }
            let Some(project_id) = app.board.selected_project_id else {
                app.ui.set_error("no project selected");
                return true;
            };
            let Some(repo_path) = state.repo_path.clone() else {
                app.ui.set_error(
                    "current directory is not a git repository (cd into a repo to add it)",
                );
                return true;
            };

            state.busy = true;
            let base_url = app.backend_url.clone();
            let net_tx = app.net_tx.clone();
            let display_name = state.suggested_project_name.clone();
            tokio::spawn(async move {
                match crate::net::ops::add_project_repository_http(
                    &base_url,
                    project_id,
                    &repo_path,
                    &display_name,
                )
                .await
                {
                    Ok(()) => {
                        let _ = net_tx
                            .send(crate::events::NetEvent::ProjectRepoAdded { project_id })
                            .await;
                        let _ = net_tx
                            .send(crate::events::NetEvent::Notice(
                                "Added repository to project.".to_string(),
                            ))
                            .await;
                    }
                    Err(e) => {
                        let _ = net_tx
                            .send(crate::events::NetEvent::Error(format!(
                                "add repository failed: {e}"
                            )))
                            .await;
                    }
                }
            });
            true
        }
        _ => false,
    }
}
