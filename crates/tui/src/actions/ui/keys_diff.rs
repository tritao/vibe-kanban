use crossterm::event::{KeyCode, KeyEvent};

use super::focus;
use crate::{
    prefs::save_prefs,
    state::{AppState, FocusPane},
    ui::components::{
        UiComponent,
        diff_list::{DiffList, DiffListEvent},
        diff_preview::{DiffPreview, DiffPreviewEvent},
        diff_repo_bar::{DiffRepoAction, DiffRepoBar, DiffRepoBarEvent},
    },
};

pub(super) fn handle_diff_key(app: &mut AppState, key: KeyEvent) -> Option<bool> {
    if app.ui.focus != FocusPane::Diff {
        return None;
    }

    if <DiffRepoBar as UiComponent>::on_event(app, DiffRepoBarEvent::Key(key)) {
        return Some(true);
    }

    if <DiffList as UiComponent>::on_event(app, DiffListEvent::Key(key)) {
        return Some(true);
    }
    if <DiffPreview as UiComponent>::on_event(app, DiffPreviewEvent::Key(key)) {
        return Some(true);
    }

    match key.code {
        KeyCode::Char('h') => {
            focus::focus_diff_files(app);
            Some(true)
        }
        KeyCode::Char('l') => {
            focus::focus_diff_preview(app);
            Some(true)
        }
        KeyCode::Char('f') => {
            crate::commands::select_files_mode(app);
            crate::diff_preview::schedule_diff_preview_refresh(
                app,
                std::time::Duration::from_millis(0),
            );
            Some(true)
        }
        KeyCode::Char('c') => {
            // Only show commits when stack mode is not enabled for this repo.
            if let Some(repo) = app.diff.repo_statuses.get(app.diff.selected_repo_index) {
                if app
                    .diff
                    .stack_status_by_repo
                    .get(&repo.repo_id)
                    .is_some_and(|s| s.available && s.enabled)
                {
                    app.ui.last_error =
                        Some("Commits view unavailable while stack mode is enabled.".to_string());
                    return Some(true);
                }
            }
            crate::commands::select_commits_mode(app);
            Some(true)
        }
        KeyCode::Char('w') => {
            app.diff.diff_wrap = !app.diff.diff_wrap;
            app.prefs.diff_wrap = app.diff.diff_wrap;
            save_prefs(&app.prefs);
            app.diff.diff_preview_cache_key = None;
            Some(true)
        }
        KeyCode::Char('u') => {
            app.diff.diff_show_untracked = !app.diff.diff_show_untracked;
            app.diff.diff_preview_cache_key = None;
            app.diff.diff_preview_cache_hash = 0;

            let rows = crate::diff::diff_rows_with_all_filtered(
                &app.diff.diff_store,
                app.diff.diff_show_untracked,
            );
            if rows.is_empty() {
                app.diff.selected_diff_index = 0;
            } else {
                app.diff.selected_diff_index = app.diff.selected_diff_index.min(rows.len() - 1);
            }
            crate::ui::sync_selected_repo_from_diff_selection(app);
            crate::diff_preview::schedule_diff_preview_refresh(
                app,
                std::time::Duration::from_millis(0),
            );
            Some(true)
        }
        KeyCode::Char('t') => {
            app.diff.diff_theme = app.diff.diff_theme.cycle_next();
            app.prefs.diff_theme = app.diff.diff_theme;
            save_prefs(&app.prefs);
            app.diff.diff_preview_cache_key = None;
            Some(true)
        }
        KeyCode::Char('d') => {
            app.diff.diff_stats_only = !app.diff.diff_stats_only;
            let _ = app.diff_stats_tx.send(app.diff.diff_stats_only);
            app.diff.diff_scroll_offset = 0;
            Some(true)
        }
        KeyCode::Char('K') => {
            crate::commands::request_stack_status_refresh(app);
            Some(true)
        }
        KeyCode::Char('E') => {
            if app.diff.repo_statuses.is_empty() {
                let _ = <DiffRepoBar as UiComponent>::on_event(
                    app,
                    DiffRepoBarEvent::Action(DiffRepoAction::RefreshStatus),
                );
                app.ui.last_error = Some("Stack: load repo status first (press S)".to_string());
                return Some(true);
            }
            let Some(repo) = app.diff.repo_statuses.get(app.diff.selected_repo_index) else {
                return Some(false);
            };
            let Some(attempt_id) = app.board.selected_attempt_id else {
                return Some(false);
            };
            crate::commands::trigger_stack_enable(app, attempt_id, repo.repo_id);
            Some(true)
        }
        KeyCode::Char('B') => {
            if app.diff.repo_statuses.is_empty() {
                let _ = <DiffRepoBar as UiComponent>::on_event(
                    app,
                    DiffRepoBarEvent::Action(DiffRepoAction::RefreshStatus),
                );
                app.ui.last_error = Some("Branches: load repo status first (press S)".to_string());
                return Some(true);
            }
            let Some(repo) = app.diff.repo_statuses.get(app.diff.selected_repo_index) else {
                return Some(false);
            };

            app.ui.branch_picker = Some(crate::state::BranchPickerState {
                mode: crate::state::BranchPickerMode::Checkout,
                repo_id: repo.repo_id,
                repo_name: repo.repo_name.clone(),
                filter: Default::default(),
                selected_index: 0,
                branches: vec![],
                busy: true,
                error: None,
            });
            let base_url = app.backend_url.clone();
            let net_tx = app.net_tx.clone();
            let repo_id = repo.repo_id;
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

            Some(true)
        }
        KeyCode::Char('T') => {
            if app.diff.repo_statuses.is_empty() {
                let _ = <DiffRepoBar as UiComponent>::on_event(
                    app,
                    DiffRepoBarEvent::Action(DiffRepoAction::RefreshStatus),
                );
                app.ui.last_error =
                    Some("Target branch: load repo status first (press S)".to_string());
                return Some(true);
            }
            let Some(repo) = app.diff.repo_statuses.get(app.diff.selected_repo_index) else {
                return Some(false);
            };

            app.ui.branch_picker = Some(crate::state::BranchPickerState {
                mode: crate::state::BranchPickerMode::ChangeTarget,
                repo_id: repo.repo_id,
                repo_name: repo.repo_name.clone(),
                filter: Default::default(),
                selected_index: 0,
                branches: vec![],
                busy: true,
                error: None,
            });
            let base_url = app.backend_url.clone();
            let net_tx = app.net_tx.clone();
            let repo_id = repo.repo_id;
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

            Some(true)
        }
        _ => None,
    }
}
