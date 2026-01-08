use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::{
    UiComponent,
    diff_list::{DiffList, DiffListEvent},
    diff_preview::{DiffPreview, DiffPreviewEvent},
    diff_repo_bar::{DiffRepoAction, DiffRepoBar, DiffRepoBarEvent},
};
use crate::{prefs::save_prefs, state::AppState};

pub(crate) enum DiffPaneEvent {
    Key(KeyEvent),
    Mouse { mouse: MouseEvent, area: Rect },
}

pub(crate) struct DiffPane;

impl UiComponent for DiffPane {
    type Event = DiffPaneEvent;

    fn render(f: &mut ratatui::Frame, app: &AppState, area: ratatui::layout::Rect) {
        let sections = crate::layout::split_diff_pane(area);

        <DiffRepoBar as UiComponent>::render(f, app, sections.repo_bar);
        <DiffList as UiComponent>::render(f, app, sections.files);
        <DiffPreview as UiComponent>::render(f, app, sections.preview);
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        match event {
            DiffPaneEvent::Key(key) => handle_diff_key(app, key),
            DiffPaneEvent::Mouse { mouse, area } => handle_diff_mouse(app, mouse, area),
        }
    }
}

fn handle_diff_mouse(app: &mut AppState, mouse: MouseEvent, area: Rect) -> bool {
    let col = mouse.column;
    let row = mouse.row;
    let split = crate::layout::split_diff_pane(area);

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if crate::layout::rect_contains(split.preview, col, row) {
                app.ui.focus_diff_preview();
                let _ = <DiffPreview as UiComponent>::on_event(
                    app,
                    DiffPreviewEvent::WheelDelta(-crate::ui::constants::DIFF_WHEEL_STEP),
                );
                return true;
            }
            if crate::layout::rect_contains(split.files, col, row) {
                app.ui.focus_diff_files();
                let _ = <DiffList as UiComponent>::on_event(app, DiffListEvent::WheelDelta(-1));
                return true;
            }
            false
        }
        MouseEventKind::ScrollDown => {
            if crate::layout::rect_contains(split.preview, col, row) {
                app.ui.focus_diff_preview();
                let _ = <DiffPreview as UiComponent>::on_event(
                    app,
                    DiffPreviewEvent::WheelDelta(crate::ui::constants::DIFF_WHEEL_STEP),
                );
                return true;
            }
            if crate::layout::rect_contains(split.files, col, row) {
                app.ui.focus_diff_files();
                let _ = <DiffList as UiComponent>::on_event(app, DiffListEvent::WheelDelta(1));
                return true;
            }
            false
        }
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            if crate::layout::rect_contains(split.repo_bar, col, row) {
                app.ui.focus_diff();
                if let Some(evt) =
                    <DiffRepoBar as UiComponent>::hit_test(app, split.repo_bar, col, row)
                {
                    let _ = <DiffRepoBar as UiComponent>::on_event(app, evt);
                }
                return true;
            }
            if crate::layout::rect_contains(split.files, col, row) {
                app.ui.focus_diff_files();
                if let Some(evt) = <DiffList as UiComponent>::hit_test(app, split.files, col, row) {
                    let _ = <DiffList as UiComponent>::on_event(app, evt);
                }
                return true;
            }
            if crate::layout::rect_contains(split.preview, col, row) {
                app.ui.focus_diff_preview();
                return true;
            }
            false
        }
        _ => false,
    }
}

fn handle_diff_key(app: &mut AppState, key: KeyEvent) -> bool {
    if <DiffRepoBar as UiComponent>::on_event(app, DiffRepoBarEvent::Key(key)) {
        return true;
    }

    if <DiffList as UiComponent>::on_event(app, DiffListEvent::Key(key)) {
        return true;
    }
    if <DiffPreview as UiComponent>::on_event(app, DiffPreviewEvent::Key(key)) {
        return true;
    }

    match key.code {
        KeyCode::Char('h') => {
            app.ui.focus_diff_files();
            return true;
        }
        KeyCode::Char('l') => {
            app.ui.focus_diff_preview();
            return true;
        }
        KeyCode::Char('f') => {
            crate::commands::select_files_mode(app);
            crate::diff_preview::schedule_diff_preview_refresh(
                app,
                crate::ui::constants::DIFF_PREVIEW_REFRESH_DELAY,
            );
            return true;
        }
        KeyCode::Char('c') => {
            // Only show commits when stack mode is not enabled for this repo.
            if let Some(repo) = crate::state::repo_scope::selected_repo(app) {
                if app
                    .diff
                    .stack_status_by_repo
                    .get(&repo.repo_id)
                    .is_some_and(|s| s.available && s.enabled)
                {
                    app.ui
                        .set_error(crate::ui::messages::errors::COMMITS_UNAVAILABLE_STACK_MODE);
                    return true;
                }
            }
            crate::commands::select_commits_mode(app);
            true
        }
        KeyCode::Char('w') => {
            app.diff.diff_wrap = !app.diff.diff_wrap;
            app.prefs.diff_wrap = app.diff.diff_wrap;
            save_prefs(&app.prefs);
            app.diff.invalidate_diff_preview_cache();
            true
        }
        KeyCode::Char('u') => {
            app.diff.diff_show_untracked = !app.diff.diff_show_untracked;
            app.diff.invalidate_diff_preview_cache();

            let rows = crate::store::diff::DiffStore::new(&app.diff.diff_store)
                .rows_with_all_filtered(app.diff.diff_show_untracked);
            app.diff.clamp_selected_diff_index(rows.len());
            sync_selected_repo_from_diff_selection(app);
            crate::diff_preview::schedule_diff_preview_refresh(
                app,
                crate::ui::constants::DIFF_PREVIEW_REFRESH_DELAY,
            );
            true
        }
        KeyCode::Char('t') => {
            app.diff.diff_theme = app.diff.diff_theme.cycle_next();
            app.prefs.diff_theme = app.diff.diff_theme;
            save_prefs(&app.prefs);
            app.diff.invalidate_diff_preview_cache();
            true
        }
        KeyCode::Char('d') => {
            app.diff.diff_stats_only = !app.diff.diff_stats_only;
            let _ = app.diff_stats_tx.send(app.diff.diff_stats_only);
            app.diff.diff_scroll.offset = 0;
            true
        }
        KeyCode::Char('K') => {
            crate::commands::request_stack_status_refresh(app);
            true
        }
        KeyCode::Char('E') => {
            if app.diff.repo_statuses.is_empty() {
                let _ = <DiffRepoBar as UiComponent>::on_event(
                    app,
                    DiffRepoBarEvent::Action(DiffRepoAction::RefreshStatus),
                );
                app.ui
                    .set_error(crate::ui::messages::errors::STACK_NEEDS_REPO_STATUS);
                return true;
            }
            let Some(repo) = crate::state::repo_scope::selected_repo(app) else {
                return false;
            };
            let Some(attempt_id) = app.board.selected_attempt_id else {
                return false;
            };
            crate::commands::trigger_stack_enable(app, attempt_id, repo.repo_id);
            true
        }
        KeyCode::Char('B') => {
            if app.diff.repo_statuses.is_empty() {
                let _ = <DiffRepoBar as UiComponent>::on_event(
                    app,
                    DiffRepoBarEvent::Action(DiffRepoAction::RefreshStatus),
                );
                app.ui
                    .set_error(crate::ui::messages::errors::BRANCHES_NEEDS_REPO_STATUS);
                return true;
            }
            let Some(repo) = crate::state::repo_scope::selected_repo(app) else {
                return false;
            };

            crate::ui::modals::open_branch_picker(
                app,
                crate::state::BranchPickerMode::Checkout,
                repo.repo_id,
                repo.repo_name.clone(),
            );

            true
        }
        KeyCode::Char('T') => {
            if app.diff.repo_statuses.is_empty() {
                let _ = <DiffRepoBar as UiComponent>::on_event(
                    app,
                    DiffRepoBarEvent::Action(DiffRepoAction::RefreshStatus),
                );
                app.ui
                    .set_error(crate::ui::messages::errors::TARGET_BRANCH_NEEDS_REPO_STATUS);
                return true;
            }
            let Some(repo) = crate::state::repo_scope::selected_repo(app) else {
                return false;
            };

            crate::ui::modals::open_branch_picker(
                app,
                crate::state::BranchPickerMode::ChangeTarget,
                repo.repo_id,
                repo.repo_name.clone(),
            );

            true
        }
        _ => false,
    }
}

fn repo_name_from_path(path: &str) -> Option<&str> {
    let first = path.split('/').next()?;
    if first.is_empty() { None } else { Some(first) }
}

fn selected_repo_status_from_diff(app: &AppState) -> Option<usize> {
    if app.diff.repo_statuses.is_empty() {
        return None;
    }
    let rows = crate::store::diff::DiffStore::new(&app.diff.diff_store)
        .rows_with_all_filtered(app.diff.diff_show_untracked);
    let selected = rows.get(app.diff.selected_diff_index)?;
    let path = selected
        .new_path
        .as_deref()
        .or(selected.old_path.as_deref())
        .unwrap_or(&selected.key);
    let repo = repo_name_from_path(path)?;
    app.diff
        .repo_statuses
        .iter()
        .position(|r| r.repo_name == repo)
}

pub(crate) fn sync_selected_repo_from_diff_selection(app: &mut AppState) {
    if let Some(idx) = selected_repo_status_from_diff(app) {
        crate::selection::change::set_selected_repo_index(app, idx);
    }
}
