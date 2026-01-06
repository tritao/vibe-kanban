use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{
    CopyTarget, Effect, composer, confirm, copy::reduce_copy, create_task, keys_board, keys_diff,
    keys_exec, keys_global, modals, sel, text_edit,
};
use crate::{
    commands::submit_composer,
    layout::current_terminal_rect,
    state::{AppState, DiffFocus, FocusPane},
};

pub(super) fn reduce_key(app: &mut AppState, key: KeyEvent) -> (bool, bool, Vec<Effect>) {
    // Alt+S toggles mouse capture (enables terminal text selection).
    if matches!(
        (key.code, key.modifiers),
        (KeyCode::Char('s'), KeyModifiers::ALT)
    ) {
        return (
            false,
            true,
            vec![Effect::SetMouseCapture(!app.ui.mouse_capture_enabled)],
        );
    }

    // Confirm modal has highest priority.
    if confirm::handle_confirm_key(app, key) {
        return (false, true, vec![]);
    }

    // Search modal.
    if app.ui.input.is_some() {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                modals::close_search(app, true);
                return (false, true, vec![]);
            }
            (KeyCode::Enter, _) => {
                modals::close_search_keep(app);
                return (false, true, vec![]);
            }
            _ => {}
        }
        if let Some((dirty, effects)) = reduce_search_key(app, key) {
            return (false, dirty, effects);
        }
        return (false, false, vec![]);
    }

    // Help modal.
    if app.ui.show_help {
        match key.code {
            KeyCode::Char('?') | KeyCode::Esc => {
                modals::close_help(app);
                return (false, true, vec![]);
            }
            _ => return (false, false, vec![]),
        }
    }

    // Branch picker modal.
    if let Some(state) = app.ui.branch_picker.as_mut() {
        match key.code {
            KeyCode::Esc => {
                app.ui.branch_picker = None;
                return (false, true, vec![]);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.selected_index = state.selected_index.saturating_sub(1);
                return (false, true, vec![]);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.selected_index = state.selected_index.saturating_add(1);
                return (false, true, vec![]);
            }
            KeyCode::Enter => {
                if state.busy {
                    return (false, false, vec![]);
                }
                let attempt_id = match app.board.selected_attempt_id {
                    Some(id) => id,
                    None => {
                        app.ui.last_error =
                            Some("No attempt selected (select a task/attempt first).".to_string());
                        return (false, true, vec![]);
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
                    app.ui.last_error = Some("No matching branches.".to_string());
                    return (false, true, vec![]);
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
                        crate::state::BranchPickerMode::Checkout => {
                            crate::net::ops::checkout_attempt_branch_http(
                                &base_url, attempt_id, &branch,
                            )
                            .await
                        }
                        crate::state::BranchPickerMode::ChangeTarget => {
                            crate::net::ops::change_target_branch_http(
                                &base_url, attempt_id, repo_id, &branch,
                            )
                            .await
                        }
                    };
                    match result {
                        Ok(()) => {
                            let _ = net_tx
                                .send(crate::events::NetEvent::Notice(format!(
                                    "{} for {repo_name}: {branch}.",
                                    match mode {
                                        crate::state::BranchPickerMode::Checkout => {
                                            "Checked out branch"
                                        }
                                        crate::state::BranchPickerMode::ChangeTarget => {
                                            "Target branch set"
                                        }
                                    }
                                )))
                                .await;
                            if let Ok(statuses) =
                                crate::net::ops::branch_status_http(&base_url, attempt_id).await
                            {
                                let _ = net_tx
                                    .send(crate::events::NetEvent::BranchStatusLoaded(statuses))
                                    .await;
                            }
                            let _ = net_tx.send(crate::events::NetEvent::DiffReconnect).await;
                        }
                        Err(e) => {
                            let label = match mode {
                                crate::state::BranchPickerMode::Checkout => "checkout branch",
                                crate::state::BranchPickerMode::ChangeTarget => {
                                    "change target branch"
                                }
                            };
                            let _ = net_tx
                                .send(crate::events::NetEvent::Error(format!(
                                    "{label} failed: {e}"
                                )))
                                .await;
                        }
                    }
                });

                // Close immediately; user will see notice/errors in main UI.
                app.ui.branch_picker = None;
                return (false, true, vec![]);
            }
            _ => {
                if !state.busy {
                    if super::text_edit::apply_text_field_key(&mut state.filter, key, false) {
                        state.selected_index = 0;
                        return (false, true, vec![]);
                    }
                }
            }
        }
        return (false, false, vec![]);
    }

    // Project setup modal (shown when no projects exist yet).
    if let Some(state) = app.ui.project_setup.as_mut() {
        match key.code {
            KeyCode::Esc => {
                app.ui.project_setup = None;
                app.ui.project_setup_dismissed = true;
                return (false, true, vec![]);
            }
            KeyCode::Enter => {
                if state.busy {
                    return (false, false, vec![]);
                }
                let Some(repo_path) = state.repo_path.clone() else {
                    app.ui.last_error = Some(
                        "current directory is not a git repository (cd into a repo to create a project)"
                            .to_string(),
                    );
                    return (false, true, vec![]);
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

                return (false, true, vec![]);
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                if state.busy {
                    return (false, false, vec![]);
                }
                if !state.has_projects {
                    return (false, false, vec![]);
                }
                let Some(project_id) = app.board.selected_project_id else {
                    app.ui.last_error = Some("no project selected".to_string());
                    return (false, true, vec![]);
                };
                let Some(repo_path) = state.repo_path.clone() else {
                    app.ui.last_error = Some(
                        "current directory is not a git repository (cd into a repo to add it)"
                            .to_string(),
                    );
                    return (false, true, vec![]);
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
                return (false, true, vec![]);
            }
            _ => return (false, false, vec![]),
        }
    }

    // Create-task modal.
    if app.ui.create_task.is_some() {
        create_task::handle_create_task_key(app, key);
        return (false, true, vec![]);
    }

    // Composer editing.
    if app.ui.composer_active {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                modals::close_composer(app);
                return (false, true, vec![]);
            }
            (KeyCode::Enter, _) => {
                if crate::slash::apply_composer_autocomplete(app) {
                    return (false, true, vec![]);
                }
                let quit = submit_composer(app);
                return (quit, true, vec![]);
            }
            _ => {}
        }
        if composer::handle_composer_key(app, key) {
            return (false, true, vec![]);
        }
        return (false, false, vec![]);
    }

    // Keymap dispatch.
    if let Some((quit, dirty)) = keys_global::handle_global_key(app, key) {
        return (quit, dirty, vec![]);
    }

    // Common selection shortcuts (independent of focus).
    match key.code {
        KeyCode::Char('[') => {
            sel::select_adjacent_attempt(app, -1);
            return (false, true, vec![]);
        }
        KeyCode::Char(']') => {
            sel::select_adjacent_attempt(app, 1);
            return (false, true, vec![]);
        }
        KeyCode::Char('x') => {
            if confirm::open_stop_exec_confirm(app) {
                return (false, true, vec![]);
            }
        }
        _ => {}
    }

    // Shift+Y copies the worktree/repo root path (independent of focus).
    if key.code == KeyCode::Char('Y') {
        return (false, false, reduce_copy(app, CopyTarget::WorktreePath));
    }

    // Shift+W copies the selected attempt's checkout path (for the selected repo).
    if key.code == KeyCode::Char('W') {
        return (
            false,
            false,
            reduce_copy(app, CopyTarget::AttemptCheckoutPath),
        );
    }

    if key.code == KeyCode::Char('y') {
        let target = match app.ui.focus {
            FocusPane::Execution => Some(CopyTarget::Execution),
            FocusPane::Diff => Some(match app.ui.diff_focus {
                DiffFocus::Files => CopyTarget::DiffFiles,
                DiffFocus::Preview => CopyTarget::DiffPreview,
            }),
            _ => None,
        };
        if let Some(target) = target {
            return (false, false, reduce_copy(app, target));
        }
    }

    if let Some(dirty) = keys_board::handle_board_key(app, key) {
        return (false, dirty, vec![]);
    }
    if let Some(dirty) = keys_diff::handle_diff_key(app, key) {
        return (false, dirty, vec![]);
    }
    if let Some((dirty, effects)) = keys_exec::handle_exec_key(app, key) {
        return (false, dirty, effects);
    }

    (false, false, vec![])
}

fn reduce_search_key(app: &mut AppState, key: KeyEvent) -> Option<(bool, Vec<Effect>)> {
    if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
        return None;
    }

    let Some(mut input) = app.ui.input.take() else {
        return None;
    };

    if !text_edit::apply_text_field_key(&mut input.field, key, false) {
        app.ui.input = Some(input);
        return None;
    }

    app.board.task_filter = input.field.buffer.clone();
    let term = current_terminal_rect();
    let area = crate::ui::layout::centered_rect(80, 25, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1);
    input.field.ensure_cursor_visible(content_w, 1);
    sel::ensure_selection_visible(app);

    app.ui.input = Some(input);
    Some((true, vec![]))
}
