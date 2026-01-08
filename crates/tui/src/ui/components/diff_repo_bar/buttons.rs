use std::time::Instant;

use crossterm::event::KeyCode;

use super::{DiffRepoAction, shared};
use crate::{
    events::GitOpKind,
    state::{AppState, RepoBranchStatus},
    store::git_status::RepoStatusRef,
    ui::button_row::ButtonSpec,
};

const GIT_SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub(super) fn action_for_key(code: KeyCode) -> Option<DiffRepoAction> {
    match code {
        KeyCode::Char('S') => Some(DiffRepoAction::RefreshStatus),
        KeyCode::Char('M') => Some(DiffRepoAction::Merge),
        KeyCode::Char('R') => Some(DiffRepoAction::Rebase),
        KeyCode::Char('P') => Some(DiffRepoAction::CreatePr),
        KeyCode::Char('C') => Some(DiffRepoAction::ResolveConflicts),
        KeyCode::Char('O') => Some(DiffRepoAction::OpenConflict),
        KeyCode::Char('A') => Some(DiffRepoAction::AbortConflicts),
        KeyCode::Char('U') | KeyCode::Enter => Some(DiffRepoAction::OpenPr),
        _ => None,
    }
}

pub(super) fn repo_bar_button_specs(
    app: &AppState,
    repo: Option<&RepoBranchStatus>,
    now: Instant,
) -> Vec<ButtonSpec<DiffRepoAction>> {
    let repo_id = repo.map(|r| r.repo_id);
    let running_kind = repo_id
        .and_then(|id| app.diff.git_ops.get(&id))
        .filter(|s| s.finished_at.is_none())
        .map(|s| s.kind)
        .or_else(|| {
            app.diff
                .git_op_global
                .as_ref()
                .filter(|s| s.finished_at.is_none())
                .map(|s| s.kind)
        });

    let done_for_repo: Option<(GitOpKind, bool, Instant)> = repo_id
        .and_then(|id| app.diff.git_ops.get(&id))
        .and_then(|s| s.finished_at.map(|t| (s.kind, s.ok.unwrap_or(false), t)));

    let done_global: Option<(GitOpKind, bool, Instant)> = app
        .diff
        .git_op_global
        .as_ref()
        .and_then(|s| s.finished_at.map(|t| (s.kind, s.ok.unwrap_or(false), t)));
    let done = done_for_repo.or(done_global);

    let has_conflicts = shared::repo_index_with_conflicts(app).is_some();

    let attempt_selected = app.board.selected_attempt_id.is_some();
    let (ahead, behind, selected_has_conflicts, pr_open, pr_url_ok, is_dirty) =
        if let Some(r) = repo {
            let r = RepoStatusRef::new(r);
            let ahead = r.commits_ahead();
            let behind = r.commits_behind();
            let selected_has_conflicts = r.has_conflicts();
            let is_dirty = r.is_dirty();
            let pr_open = r.pr_number();
            let pr_url_ok = r.pr_url().is_some_and(|u| !u.trim().is_empty());
            (
                ahead,
                behind,
                selected_has_conflicts,
                pr_open,
                pr_url_ok,
                is_dirty,
            )
        } else {
            (0, 0, false, None, false, false)
        };

    let is_applicable = |action: DiffRepoAction| -> bool {
        match action {
            DiffRepoAction::RefreshStatus => attempt_selected,
            DiffRepoAction::ResolveConflicts
            | DiffRepoAction::OpenConflict
            | DiffRepoAction::AbortConflicts => attempt_selected && has_conflicts,
            DiffRepoAction::Merge => {
                attempt_selected
                    && repo.is_some()
                    && ahead > 0
                    && !selected_has_conflicts
                    && !is_dirty
            }
            DiffRepoAction::Rebase => {
                attempt_selected
                    && repo.is_some()
                    && behind > 0
                    && !selected_has_conflicts
                    && !is_dirty
            }
            DiffRepoAction::CreatePr => {
                attempt_selected
                    && repo.is_some()
                    && ahead > 0
                    && pr_open.is_none()
                    && !selected_has_conflicts
                    && !is_dirty
            }
            DiffRepoAction::OpenPr => {
                attempt_selected && repo.is_some() && pr_open.is_some() && pr_url_ok
            }
        }
    };

    let mut base: Vec<(DiffRepoAction, &'static str)> = vec![];
    if has_conflicts {
        base.push((DiffRepoAction::ResolveConflicts, "[C]Resolve"));
        base.push((DiffRepoAction::OpenConflict, "[O]pen"));
        base.push((DiffRepoAction::AbortConflicts, "[A]bort"));
    }
    base.extend([
        (DiffRepoAction::Merge, "[M]erge"),
        if pr_open.is_some() {
            (DiffRepoAction::OpenPr, "[U]OpenPR")
        } else {
            (DiffRepoAction::CreatePr, "[P]R")
        },
        (DiffRepoAction::Rebase, "[R]ebase"),
        (DiffRepoAction::RefreshStatus, "[S]tatus"),
    ]);

    base.into_iter()
        .map(|(action, label)| {
            let kind = shared::git_kind_for_diff_action(action);

            let recently_done = done.is_some_and(|(done_kind, _, done_at)| {
                kind == Some(done_kind)
                    && now.saturating_duration_since(done_at) < crate::ui::constants::TOAST_SHORT
            });

            let is_running = kind.is_some_and(|k| running_kind == Some(k));
            let any_running = running_kind.is_some();
            let enabled_for_ops = if kind.is_some() {
                !any_running || is_running
            } else {
                !any_running
            };
            let enabled = enabled_for_ops && is_applicable(action);

            let mut rendered_label = label.to_string();
            if is_running && kind.is_some() {
                let started_at = repo_id
                    .and_then(|id| app.diff.git_ops.get(&id))
                    .and_then(|s| kind.filter(|k| s.kind == *k).map(|_| s.started_at))
                    .or_else(|| {
                        app.diff
                            .git_op_global
                            .as_ref()
                            .and_then(|s| kind.filter(|k| s.kind == *k).map(|_| s.started_at))
                    })
                    .unwrap_or(now);
                let elapsed = now.saturating_duration_since(started_at);
                let secs = elapsed.as_secs().max(1);
                let frame = GIT_SPINNER_FRAMES
                    [((elapsed.as_millis() / 90) as usize) % GIT_SPINNER_FRAMES.len()];
                rendered_label = format!("{label}… {frame} {secs}s");
            } else if recently_done {
                let ok = done.map(|(_, ok, _)| ok).unwrap_or(true);
                rendered_label = if ok {
                    format!("{label} ✓")
                } else {
                    format!("{label} !")
                };
            }

            let done_ok = done.map(|(_, ok, _)| ok).unwrap_or(true);
            let style = crate::ui::palette::diff_repo_action_button_style(
                action,
                enabled,
                recently_done && kind.is_some() && !done_ok,
            );

            ButtonSpec {
                id: action,
                label: rendered_label,
                style,
                enabled,
            }
        })
        .collect()
}
