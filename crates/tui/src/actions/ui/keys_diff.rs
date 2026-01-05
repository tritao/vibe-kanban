use crossterm::event::{KeyCode, KeyEvent};

use super::{focus, scroll, sel};
use crate::{
    prefs::save_prefs,
    state::{AppState, DiffFocus, FocusPane},
    ui::{DiffRepoAction, trigger_diff_repo_action},
};

pub(super) fn handle_diff_key(app: &mut AppState, key: KeyEvent) -> Option<bool> {
    if app.ui.focus != FocusPane::Diff {
        return None;
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
        KeyCode::Char('w') => {
            app.diff.diff_wrap = !app.diff.diff_wrap;
            app.prefs.diff_wrap = app.diff.diff_wrap;
            save_prefs(&app.prefs);
            app.diff.diff_preview_cache_key = None;
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
        KeyCode::Char('S') => {
            trigger_diff_repo_action(app, DiffRepoAction::RefreshStatus);
            Some(true)
        }
        KeyCode::Char('M') => {
            trigger_diff_repo_action(app, DiffRepoAction::Merge);
            Some(true)
        }
        KeyCode::Char('R') => {
            trigger_diff_repo_action(app, DiffRepoAction::Rebase);
            Some(true)
        }
        KeyCode::Char('P') => {
            trigger_diff_repo_action(app, DiffRepoAction::CreatePr);
            Some(true)
        }
        KeyCode::Char('C') => {
            trigger_diff_repo_action(app, DiffRepoAction::ResolveConflicts);
            Some(true)
        }
        KeyCode::Char('O') => {
            trigger_diff_repo_action(app, DiffRepoAction::OpenConflict);
            Some(true)
        }
        KeyCode::Char('A') => {
            trigger_diff_repo_action(app, DiffRepoAction::AbortConflicts);
            Some(true)
        }
        KeyCode::Char('U') | KeyCode::Enter => {
            trigger_diff_repo_action(app, DiffRepoAction::OpenPr);
            Some(true)
        }
        KeyCode::Up | KeyCode::Char('k') if app.ui.diff_focus == DiffFocus::Files => {
            sel::select_adjacent_diff_file(app, -1);
            Some(true)
        }
        KeyCode::Down | KeyCode::Char('j') if app.ui.diff_focus == DiffFocus::Files => {
            sel::select_adjacent_diff_file(app, 1);
            Some(true)
        }
        KeyCode::PageUp => {
            scroll::scroll_diff_up(app, 20);
            Some(true)
        }
        KeyCode::PageDown => {
            scroll::scroll_diff_down(app, 20);
            Some(true)
        }
        _ => None,
    }
}
