use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::UiComponent;
use crate::{
    commands::{
        begin_git_op, open_url, request_branch_status_refresh, resolve_repo_for_command, set_toast,
        trigger_abort_conflicts,
    },
    events::{GitOpKind, NetEvent},
    layout::{compute_main_layout, current_terminal_rect, rect_contains},
    net::ops::{
        CreateGitHubPrRequest, branch_status_http, create_pr_http, merge_task_attempt_http,
        open_editor_http, rebase_task_attempt_http,
    },
    selection::find_task,
    state::{
        AppState, FocusPane, Merge, MergeStatus, RepoBranchStatus,
        build_resolve_conflicts_instructions,
    },
    text::{display_width, truncate_to_width},
};

pub(crate) enum DiffRepoBarEvent {
    Key(KeyEvent),
    Action(DiffRepoAction),
}

pub(crate) struct DiffRepoBar;

fn selected_attempt_branch(app: &AppState) -> String {
    app.board
        .selected_attempt_id
        .and_then(|id| app.board.attempts.iter().find(|a| a.id == id))
        .map(|a| a.branch.clone())
        .unwrap_or_else(|| "—".to_string())
}

fn repo_index_with_conflicts(app: &AppState) -> Option<usize> {
    let selected = app.diff.repo_statuses.get(app.diff.selected_repo_index);
    if selected
        .is_some_and(|r| r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty())
    {
        return Some(app.diff.selected_repo_index);
    }
    app.diff
        .repo_statuses
        .iter()
        .position(|r| r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty())
}

fn badge(text: impl Into<String>, fg: Color, bg: Color) -> Span<'static> {
    crate::ui::widgets::badge(text, fg, bg)
}

fn pr_badge_style(status: MergeStatus) -> (Color, Color) {
    match status {
        MergeStatus::Open => (Color::Black, Color::LightBlue),
        MergeStatus::Merged => (Color::Black, Color::LightGreen),
        MergeStatus::Closed => (Color::White, Color::Red),
        MergeStatus::Unknown => (Color::Black, Color::LightYellow),
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DiffRepoAction {
    Merge,
    CreatePr,
    OpenPr,
    Rebase,
    ResolveConflicts,
    OpenConflict,
    AbortConflicts,
    RefreshStatus,
}

fn git_kind_for_diff_action(action: DiffRepoAction) -> Option<GitOpKind> {
    match action {
        DiffRepoAction::RefreshStatus => Some(GitOpKind::Status),
        DiffRepoAction::Merge => Some(GitOpKind::Merge),
        DiffRepoAction::Rebase => Some(GitOpKind::Rebase),
        DiffRepoAction::CreatePr => Some(GitOpKind::CreatePr),
        DiffRepoAction::AbortConflicts => Some(GitOpKind::Abort),
        DiffRepoAction::ResolveConflicts
        | DiffRepoAction::OpenConflict
        | DiffRepoAction::OpenPr => None,
    }
}

#[derive(Clone)]
struct RepoBarButtonSpec {
    action: DiffRepoAction,
    label: String,
    style: Style,
    enabled: bool,
}

const GIT_SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn repo_bar_button_specs(
    app: &AppState,
    repo: Option<&RepoBranchStatus>,
    now: Instant,
) -> Vec<RepoBarButtonSpec> {
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

    let has_conflicts = repo_index_with_conflicts(app).is_some();

    let attempt_selected = app.board.selected_attempt_id.is_some();
    let (ahead, behind, selected_has_conflicts, pr_open, pr_url, is_dirty) = if let Some(r) = repo {
        let ahead = r.status.commits_ahead.unwrap_or(0);
        let behind = r.status.commits_behind.unwrap_or(0);
        let selected_has_conflicts =
            r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty();
        let is_dirty = r.status.has_uncommitted_changes.unwrap_or(false)
            || r.status.uncommitted_count.unwrap_or(0) > 0
            || r.status.untracked_count.unwrap_or(0) > 0;
        let pr_open = r.status.merges.iter().find_map(|m| match m {
            Merge::Pr(pr) => Some(pr.pr_info.number),
            _ => None,
        });
        let pr_url = r.status.merges.iter().find_map(|m| match m {
            Merge::Pr(pr) => Some(pr.pr_info.url.clone()),
            _ => None,
        });
        (
            ahead,
            behind,
            selected_has_conflicts,
            pr_open,
            pr_url,
            is_dirty,
        )
    } else {
        (0, 0, false, None, None, false)
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
                attempt_selected
                    && repo.is_some()
                    && pr_open.is_some()
                    && pr_url.as_ref().is_some_and(|u| !u.trim().is_empty())
            }
        }
    };

    let mut base: Vec<(DiffRepoAction, &'static str, Color)> = vec![];
    if has_conflicts {
        base.push((DiffRepoAction::ResolveConflicts, "[C]Resolve", Color::Cyan));
        base.push((DiffRepoAction::OpenConflict, "[O]pen", Color::Blue));
        base.push((DiffRepoAction::AbortConflicts, "[A]bort", Color::Red));
    }
    base.extend([
        (DiffRepoAction::Merge, "[M]erge", Color::Green),
        if pr_open.is_some() {
            (DiffRepoAction::OpenPr, "[U]OpenPR", Color::Blue)
        } else {
            (DiffRepoAction::CreatePr, "[P]R", Color::Blue)
        },
        (DiffRepoAction::Rebase, "[R]ebase", Color::Yellow),
        (DiffRepoAction::RefreshStatus, "[S]tatus", Color::Cyan),
    ]);

    base.into_iter()
        .map(|(action, label, color)| {
            let kind = git_kind_for_diff_action(action);

            let recently_done = done.is_some_and(|(done_kind, _, done_at)| {
                kind == Some(done_kind)
                    && now.saturating_duration_since(done_at) < Duration::from_secs(2)
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

            let mut style = Style::default().fg(color).add_modifier(Modifier::BOLD);
            if !enabled {
                style = style.add_modifier(Modifier::DIM);
            }
            let done_ok = done.map(|(_, ok, _)| ok).unwrap_or(true);
            if recently_done && kind.is_some() && !done_ok {
                style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
            }

            RepoBarButtonSpec {
                action,
                label: rendered_label,
                style,
                enabled,
            }
        })
        .collect()
}

pub(crate) fn diff_repo_bar_action_at(
    app: &AppState,
    area: Rect,
    col: u16,
    row: u16,
) -> Option<DiffRepoAction> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let inner_x0 = area.x.saturating_add(1);
    let inner_x1 = area.x.saturating_add(area.width).saturating_sub(1);
    let inner_y0 = area.y.saturating_add(1);
    let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
    if col < inner_x0 || col >= inner_x1 || row < inner_y0 || row >= inner_y1 {
        return None;
    }

    let w = area.width.saturating_sub(2) as usize;
    let repo = app.diff.repo_statuses.get(
        app.diff
            .selected_repo_index
            .min(app.diff.repo_statuses.len().saturating_sub(1)),
    );
    let branch = selected_attempt_branch(app);

    let (
        repo_name,
        target_branch,
        ahead,
        behind,
        remote_ahead,
        remote_behind,
        dirty,
        untracked,
        conflicts,
        pr_open,
        stack_badge_plain,
    ) = if let Some(r) = repo {
        let ahead = r.status.commits_ahead.unwrap_or(0);
        let behind = r.status.commits_behind.unwrap_or(0);
        let remote_ahead = r.status.remote_commits_ahead.unwrap_or(0);
        let remote_behind = r.status.remote_commits_behind.unwrap_or(0);
        let dirty = r.status.uncommitted_count.unwrap_or(0);
        let untracked = r.status.untracked_count.unwrap_or(0);
        let conflicts = r.status.conflicted_files.len();
        let pr_open = r.status.merges.iter().find_map(|m| match m {
            Merge::Pr(pr) => Some(pr.pr_info.number),
            _ => None,
        });
        let stack_badge_plain = selected_stack_badge(app, repo).map(|(plain, _)| plain);
        (
            r.repo_name.clone(),
            r.status.target_branch_name.clone(),
            ahead,
            behind,
            remote_ahead,
            remote_behind,
            dirty,
            untracked,
            conflicts,
            pr_open,
            stack_badge_plain,
        )
    } else {
        (
            "(repo)".to_string(),
            "—".to_string(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            None,
            None,
        )
    };

    let left_base = if repo.is_some() {
        format!("{repo_name}  {branch} → {target_branch}")
    } else if app.board.selected_attempt_id.is_some() {
        format!("{branch}  (press S for repo status)")
    } else {
        "(no attempt)".to_string()
    };

    let now = Instant::now();
    let buttons = repo_bar_button_specs(app, repo, now);

    let mut right_plain = String::new();
    let mut any_badge = false;
    if dirty > 0 {
        right_plain.push_str(&format!(" Δ{dirty} "));
        any_badge = true;
    }
    if untracked > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" ?{untracked} "));
        any_badge = true;
    }
    if ahead > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" +{ahead} "));
        any_badge = true;
    }
    if behind > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" {behind} "));
        any_badge = true;
    }
    if remote_ahead > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" r+{remote_ahead} "));
        any_badge = true;
    }
    if remote_behind > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" r-{remote_behind} "));
        any_badge = true;
    }
    if conflicts > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" !{conflicts} "));
        any_badge = true;
    }
    if let Some(n) = pr_open {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" PR#{n} "));
        any_badge = true;
    }
    if let Some(plain) = stack_badge_plain.as_deref() {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(plain);
        any_badge = true;
    }
    if any_badge {
        right_plain.push_str("  ");
    }
    right_plain.push_str(
        &buttons
            .iter()
            .map(|b| b.label.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    );
    let right_w = display_width(&right_plain);

    let can_show_right = w > right_w + 2;
    if !can_show_right {
        return None;
    }

    let left_w = w - right_w - 2;
    let left = truncate_to_width(&left_base, left_w);

    let inner_col = col.saturating_sub(inner_x0) as usize;
    let mut cursor = display_width(&left);
    cursor = cursor.saturating_add(2); // after "  "

    let mut first_badge = true;
    let mut push_badge_plain = |plain: &str| {
        if !first_badge {
            cursor = cursor.saturating_add(1);
        }
        first_badge = false;
        cursor = cursor.saturating_add(display_width(plain));
    };

    if dirty > 0 {
        push_badge_plain(&format!(" Δ{dirty} "));
    }
    if untracked > 0 {
        push_badge_plain(&format!(" ?{untracked} "));
    }
    if ahead > 0 {
        push_badge_plain(&format!(" +{ahead} "));
    }
    if behind > 0 {
        push_badge_plain(&format!(" {behind} "));
    }
    if remote_ahead > 0 {
        push_badge_plain(&format!(" r+{remote_ahead} "));
    }
    if remote_behind > 0 {
        push_badge_plain(&format!(" r-{remote_behind} "));
    }
    if conflicts > 0 {
        push_badge_plain(&format!(" !{conflicts} "));
    }
    if let Some(n) = pr_open {
        push_badge_plain(&format!(" PR#{n} "));
    }
    if let Some(plain) = stack_badge_plain.as_deref() {
        push_badge_plain(plain);
    }
    if !first_badge {
        cursor = cursor.saturating_add(2); // before buttons
    }

    for (idx, b) in buttons.iter().enumerate() {
        let start = cursor;
        let end = start.saturating_add(display_width(&b.label));
        if inner_col >= start && inner_col < end {
            if !b.enabled {
                return None;
            }
            return Some(b.action);
        }
        cursor = end;
        if idx + 1 < buttons.len() {
            cursor = cursor.saturating_add(1);
        }
    }

    None
}

pub(crate) fn trigger_diff_repo_action(app: &mut AppState, action: DiffRepoAction) {
    match action {
        DiffRepoAction::ResolveConflicts => {
            let Some(_attempt_id) = app.board.selected_attempt_id else {
                set_toast(
                    app,
                    "Conflicts: no attempt selected".to_string(),
                    Color::Red,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            };
            if app.diff.repo_statuses.is_empty() {
                request_branch_status_refresh(app);
                set_toast(
                    app,
                    "Conflicts: loading repo status…".to_string(),
                    Color::Yellow,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            }

            let Some(idx) = repo_index_with_conflicts(app) else {
                set_toast(
                    app,
                    "Conflicts: none detected".to_string(),
                    Color::Green,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            };
            crate::selection_hooks::set_selected_repo_index(app, idx);
            let repo = match app.diff.repo_statuses.get(idx) {
                Some(r) => r,
                None => return,
            };

            let attempt_branch = selected_attempt_branch(app);
            let instructions = build_resolve_conflicts_instructions(
                Some(&attempt_branch),
                Some(&repo.status.target_branch_name),
                &repo.status.conflicted_files,
                repo.status.conflict_op,
                Some(&repo.repo_name),
            );

            app.ui.focus = FocusPane::Execution;
            app.ui.composer_active = true;
            app.ui.composer_suggest_index = 0;
            app.ui.refresh_branch_status_after_send = true;
            app.ui.composer.buffer = instructions;
            app.ui.composer.set_end();
            let layout = compute_main_layout(current_terminal_rect(), FocusPane::Execution);
            let area = layout.exec_input;
            let inner_w = area.width.saturating_sub(2) as usize;
            let inner_h = area.height.saturating_sub(2) as usize;
            let prefix_w = crate::text::display_width("  ");
            let content_w = inner_w.saturating_sub(prefix_w).saturating_sub(1).max(1);
            app.ui
                .composer
                .ensure_cursor_visible(content_w, inner_h.max(1));
            set_toast(
                app,
                format!("Conflicts: drafted resolution request ({})", repo.repo_name),
                Color::Cyan,
                Some(Instant::now() + Duration::from_secs(2)),
            );
        }
        DiffRepoAction::OpenConflict => {
            let Some(attempt_id) = app.board.selected_attempt_id else {
                set_toast(
                    app,
                    "Open: no attempt selected".to_string(),
                    Color::Red,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            };
            if app.diff.repo_statuses.is_empty() {
                request_branch_status_refresh(app);
                set_toast(
                    app,
                    "Open: loading repo status…".to_string(),
                    Color::Yellow,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            }

            let Some(idx) = repo_index_with_conflicts(app) else {
                set_toast(
                    app,
                    "Open: no conflicts detected".to_string(),
                    Color::Green,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            };
            crate::selection_hooks::set_selected_repo_index(app, idx);
            let Some(repo) = app.diff.repo_statuses.get(idx) else {
                return;
            };
            let Some(first) = repo.status.conflicted_files.first().cloned() else {
                set_toast(
                    app,
                    "Open: no conflicted files listed".to_string(),
                    Color::Yellow,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            };

            let repo_name = repo.repo_name.clone();
            crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
                match open_editor_http(&base_url, attempt_id, Some(first.clone())).await {
                    Ok(url) => {
                        let msg = match url {
                            Some(url) => {
                                format!("Opened conflict file ({repo_name}): {first} ({url})")
                            }
                            None => format!("Opened conflict file ({repo_name}): {first}"),
                        };
                        let _ = net_tx.send(NetEvent::Notice(msg)).await;
                    }
                    Err(e) => {
                        let _ = net_tx
                            .send(NetEvent::Error(format!("open editor failed: {e}")))
                            .await;
                    }
                }
            });
        }
        DiffRepoAction::OpenPr => {
            let Some(repo) = app.diff.repo_statuses.get(
                app.diff
                    .selected_repo_index
                    .min(app.diff.repo_statuses.len().saturating_sub(1)),
            ) else {
                set_toast(
                    app,
                    "PR: no repo selected".to_string(),
                    Color::Yellow,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            };
            let Some(pr) = repo.status.merges.iter().find_map(|m| match m {
                Merge::Pr(pr) => Some(&pr.pr_info),
                _ => None,
            }) else {
                set_toast(
                    app,
                    "PR: none attached".to_string(),
                    Color::Yellow,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            };

            match open_url(&pr.url) {
                Ok(()) => {
                    set_toast(
                        app,
                        format!("PR: opened (PR#{})", pr.number),
                        Color::Green,
                        Some(Instant::now() + Duration::from_secs(2)),
                    );
                }
                Err(e) => {
                    app.ui.last_notice = Some(format!("PR URL: {}", pr.url));
                    set_toast(
                        app,
                        format!("PR: failed to open ({e})"),
                        Color::Red,
                        Some(Instant::now() + Duration::from_secs(3)),
                    );
                }
            }
        }
        DiffRepoAction::AbortConflicts => {
            let Some(attempt_id) = app.board.selected_attempt_id else {
                set_toast(
                    app,
                    "Abort: no attempt selected".to_string(),
                    Color::Red,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            };
            if app.diff.repo_statuses.is_empty() {
                request_branch_status_refresh(app);
                set_toast(
                    app,
                    "Abort: loading repo status…".to_string(),
                    Color::Yellow,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            }

            let Some(idx) = repo_index_with_conflicts(app) else {
                set_toast(
                    app,
                    "Abort: no conflicts detected".to_string(),
                    Color::Green,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            };
            crate::selection_hooks::set_selected_repo_index(app, idx);
            let Some(repo) = app.diff.repo_statuses.get(idx) else {
                return;
            };
            let repo_id = repo.repo_id;
            let repo_name = repo.repo_name.clone();
            trigger_abort_conflicts(app, attempt_id, repo_id, &repo_name);
        }
        DiffRepoAction::RefreshStatus => {
            let repo = app.diff.repo_statuses.get(
                app.diff
                    .selected_repo_index
                    .min(app.diff.repo_statuses.len().saturating_sub(1)),
            );
            let (repo_id, repo_name) = match repo {
                Some(r) => (Some(r.repo_id), r.repo_name.clone()),
                None => (None, String::new()),
            };

            if !begin_git_op(app, repo_id, GitOpKind::Status, &repo_name) {
                return;
            }

            let attempt_id = app.board.selected_attempt_id;
            let task_id = app.board.selected_task_id;
            let project_id = app.board.selected_project_id;
            let executor_profile = app.ui.selected_executor_profile.clone();
            crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
                let attempt_id = match crate::commands::ensure_attempt_id_for_repo_ops(
                    &base_url,
                    &net_tx,
                    attempt_id,
                    task_id,
                    project_id,
                    executor_profile,
                )
                .await
                {
                    Some(id) => id,
                    None => {
                        let _ = net_tx
                            .send(NetEvent::GitOpFinished {
                                repo_id,
                                kind: GitOpKind::Status,
                                ok: false,
                                message: "Git: status failed".to_string(),
                            })
                            .await;
                        return;
                    }
                };

                match branch_status_http(&base_url, attempt_id).await {
                    Ok(statuses) => {
                        let _ = net_tx
                            .send(NetEvent::BranchStatusLoaded {
                                attempt_id,
                                statuses,
                            })
                            .await;
                        let _ = net_tx
                            .send(NetEvent::GitOpFinished {
                                repo_id,
                                kind: GitOpKind::Status,
                                ok: true,
                                message: if repo_name.is_empty() {
                                    "Git: status updated".to_string()
                                } else {
                                    format!("Git: status updated ({repo_name})")
                                },
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = net_tx
                            .send(NetEvent::Error(crate::fmt::op_failed("branch status", e)))
                            .await;
                        let _ = net_tx
                            .send(NetEvent::GitOpFinished {
                                repo_id,
                                kind: GitOpKind::Status,
                                ok: false,
                                message: "Git: status failed".to_string(),
                            })
                            .await;
                    }
                }
            });
        }
        DiffRepoAction::Merge => {
            let Ok((repo_id, repo_name)) = resolve_repo_for_command(app, None) else {
                return;
            };
            let Some(attempt_id) = app.board.selected_attempt_id else {
                return;
            };
            let Some(r) = app.diff.repo_statuses.iter().find(|r| r.repo_id == repo_id) else {
                return;
            };
            if r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty() {
                set_toast(
                    app,
                    "Merge: conflicts in progress (resolve/abort first)".to_string(),
                    Color::Yellow,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            }
            if r.status.commits_ahead.unwrap_or(0) == 0 {
                set_toast(
                    app,
                    "Merge: nothing to merge (up to date)".to_string(),
                    Color::Green,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            }
            crate::commands::spawn_repo_git_op(
                app,
                attempt_id,
                repo_id,
                GitOpKind::Merge,
                &repo_name,
                crate::commands::GitOpOutcome {
                    notice: Some(format!("Merged {repo_name}.")),
                    refresh_branch_status: true,
                    diff_reconnect: false,
                    finished_message_ok: Some(format!("Git: merge finished ({repo_name})")),
                    finished_message_err: Some(format!("Git: merge failed ({repo_name})")),
                },
                move |base_url| async move {
                    merge_task_attempt_http(&base_url, attempt_id, repo_id).await
                },
            );
        }
        DiffRepoAction::Rebase => {
            let Ok((repo_id, repo_name)) = resolve_repo_for_command(app, None) else {
                return;
            };
            let Some(attempt_id) = app.board.selected_attempt_id else {
                return;
            };
            let Some(r) = app.diff.repo_statuses.iter().find(|r| r.repo_id == repo_id) else {
                return;
            };
            if r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty() {
                set_toast(
                    app,
                    "Rebase: conflicts in progress (resolve/abort first)".to_string(),
                    Color::Yellow,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            }
            if r.status.commits_behind.unwrap_or(0) == 0 {
                set_toast(
                    app,
                    "Rebase: already up to date".to_string(),
                    Color::Green,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            }
            crate::commands::spawn_repo_git_op(
                app,
                attempt_id,
                repo_id,
                GitOpKind::Rebase,
                &repo_name,
                crate::commands::GitOpOutcome {
                    notice: Some(format!("Rebase started for {repo_name}.")),
                    refresh_branch_status: true,
                    diff_reconnect: true,
                    finished_message_ok: Some(format!("Git: rebase finished ({repo_name})")),
                    finished_message_err: Some(format!("Git: rebase failed ({repo_name})")),
                },
                move |base_url| async move {
                    rebase_task_attempt_http(&base_url, attempt_id, repo_id, None, None).await
                },
            );
        }
        DiffRepoAction::CreatePr => {
            let Ok((repo_id, repo_name)) = resolve_repo_for_command(app, None) else {
                return;
            };
            let Some(attempt_id) = app.board.selected_attempt_id else {
                return;
            };
            let Some(r) = app.diff.repo_statuses.iter().find(|r| r.repo_id == repo_id) else {
                return;
            };
            if r.status.is_rebase_in_progress || !r.status.conflicted_files.is_empty() {
                set_toast(
                    app,
                    "PR: conflicts in progress (resolve/abort first)".to_string(),
                    Color::Yellow,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            }
            if r.status.commits_ahead.unwrap_or(0) == 0 {
                set_toast(
                    app,
                    "PR: no changes to open (up to date)".to_string(),
                    Color::Green,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            }
            let pr_open = r.status.merges.iter().find_map(|m| match m {
                Merge::Pr(pr) => Some(pr.pr_info.number),
                _ => None,
            });
            if let Some(n) = pr_open {
                set_toast(
                    app,
                    format!("PR: already exists (PR#{n})"),
                    Color::Green,
                    Some(Instant::now() + Duration::from_secs(2)),
                );
                return;
            }
            if !begin_git_op(app, Some(repo_id), GitOpKind::CreatePr, &repo_name) {
                return;
            }
            let title = app
                .board
                .selected_task_id
                .and_then(|id| find_task(&app.board.tasks_store, id).map(|t| t.title))
                .unwrap_or_else(|| "Vibe Kanban PR".to_string());
            crate::commands::spawn_net_task(app, move |base_url, net_tx| async move {
                match create_pr_http(
                    &base_url,
                    attempt_id,
                    CreateGitHubPrRequest {
                        title,
                        body: None,
                        target_branch: None,
                        draft: Some(false),
                        repo_id,
                        auto_generate_description: false,
                    },
                )
                .await
                {
                    Ok(url) => {
                        let _ = net_tx
                            .send(NetEvent::Notice(format!(
                                "PR created for {repo_name}: {url}"
                            )))
                            .await;
                        if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                            let _ = net_tx
                                .send(NetEvent::BranchStatusLoaded {
                                    attempt_id,
                                    statuses,
                                })
                                .await;
                        }
                        let _ = net_tx
                            .send(NetEvent::GitOpFinished {
                                repo_id: Some(repo_id),
                                kind: GitOpKind::CreatePr,
                                ok: true,
                                message: format!("Git: PR created ({repo_name})"),
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = net_tx
                            .send(NetEvent::Error(format!("pr create failed: {e}")))
                            .await;
                        let _ = net_tx
                            .send(NetEvent::GitOpFinished {
                                repo_id: Some(repo_id),
                                kind: GitOpKind::CreatePr,
                                ok: false,
                                message: format!("Git: PR create failed ({repo_name})"),
                            })
                            .await;
                    }
                }
            });
        }
    }
}

fn selected_stack_badge(
    app: &AppState,
    repo: Option<&RepoBranchStatus>,
) -> Option<(String, Span<'static>)> {
    let repo_id = repo?.repo_id;
    let status = app.diff.stack_status_by_repo.get(&repo_id)?;
    if !status.available {
        return Some((
            " Stack: missing ".to_string(),
            badge("Stack: missing", Color::White, Color::Red),
        ));
    }
    if !status.enabled {
        return Some((
            " Stack: off ".to_string(),
            badge("Stack: off", Color::Black, Color::LightYellow),
        ));
    }
    let current = status
        .patches
        .iter()
        .find(|p| p.is_current)
        .map(|p| p.name.as_str())
        .unwrap_or("?");
    let applied = status
        .patches
        .iter()
        .filter(|p| p.state == "applied")
        .count();
    let total = status.patches.len();
    let label = format!("Stack: {applied}/{total} [{current}]");
    Some((
        format!(" {label} "),
        badge(label, Color::Black, Color::LightGreen),
    ))
}

pub(crate) fn render_diff_repo_bar(f: &mut Frame, app: &AppState, area: Rect) {
    let border_style = if app.ui.focus == FocusPane::Diff {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let w = area.width.saturating_sub(2) as usize;

    let repo = app.diff.repo_statuses.get(
        app.diff
            .selected_repo_index
            .min(app.diff.repo_statuses.len().saturating_sub(1)),
    );
    let branch = selected_attempt_branch(app);

    let (
        repo_name,
        target_branch,
        ahead,
        behind,
        remote_ahead,
        remote_behind,
        dirty,
        untracked,
        conflicts,
        pr_open,
    ) = if let Some(r) = repo {
        let ahead = r.status.commits_ahead.unwrap_or(0);
        let behind = r.status.commits_behind.unwrap_or(0);
        let remote_ahead = r.status.remote_commits_ahead.unwrap_or(0);
        let remote_behind = r.status.remote_commits_behind.unwrap_or(0);
        let dirty_count = r.status.uncommitted_count.unwrap_or(0);
        let untracked = r.status.untracked_count.unwrap_or(0);
        let conflicts = r.status.conflicted_files.len();
        let pr_open = r.status.merges.iter().find_map(|m| match m {
            Merge::Pr(pr) => Some((pr.pr_info.number, pr.pr_info.status)),
            _ => None,
        });
        (
            r.repo_name.clone(),
            r.status.target_branch_name.clone(),
            ahead,
            behind,
            remote_ahead,
            remote_behind,
            dirty_count,
            untracked,
            conflicts,
            pr_open,
        )
    } else {
        (
            "(repo)".to_string(),
            "—".to_string(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            None,
        )
    };

    let left_base = if repo.is_some() {
        format!("{repo_name}  {branch} → {target_branch}")
    } else if app.board.selected_attempt_id.is_some() {
        format!("{branch}  (press S for repo status)")
    } else {
        "(no attempt)".to_string()
    };

    let now = Instant::now();
    let buttons = repo_bar_button_specs(app, repo, now);
    let stack_badge = selected_stack_badge(app, repo);

    let mut right_plain = String::new();
    let mut any_badge = false;
    if dirty > 0 {
        right_plain.push_str(&format!(" Δ{dirty} "));
        any_badge = true;
    }
    if untracked > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" ?{untracked} "));
        any_badge = true;
    }
    if ahead > 0 {
        right_plain.push_str(&format!(" +{ahead} "));
        any_badge = true;
    }
    if behind > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" {behind} "));
        any_badge = true;
    }
    if remote_ahead > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" r+{remote_ahead} "));
        any_badge = true;
    }
    if remote_behind > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" r-{remote_behind} "));
        any_badge = true;
    }
    if conflicts > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" !{conflicts} "));
        any_badge = true;
    }
    if let Some((n, _)) = pr_open {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" PR#{n} "));
        any_badge = true;
    }
    if let Some((plain, _)) = stack_badge.as_ref() {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(plain);
        any_badge = true;
    }
    if any_badge {
        right_plain.push_str("  ");
    }
    right_plain.push_str(
        &buttons
            .iter()
            .map(|b| b.label.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    );
    let right_w = display_width(&right_plain);

    let can_show_right = w > right_w + 2;
    let left_w = if can_show_right { w - right_w - 2 } else { w };
    let left = truncate_to_width(&left_base, left_w);

    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        left,
        Style::default().add_modifier(Modifier::BOLD),
    )];

    if can_show_right {
        spans.push(Span::raw("  "));
        let mut first = true;
        if dirty > 0 {
            spans.push(badge(format!("Δ{dirty}"), Color::Black, Color::LightYellow));
            first = false;
        }
        if untracked > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            spans.push(badge(
                format!("?{untracked}"),
                Color::Black,
                Color::LightCyan,
            ));
            first = false;
        }
        if ahead > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            spans.push(badge(format!("+{ahead}"), Color::Black, Color::LightGreen));
            first = false;
        }
        if behind > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            spans.push(badge(format!("{behind}"), Color::Black, Color::LightYellow));
            first = false;
        }
        if remote_ahead > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            spans.push(badge(
                format!("r+{remote_ahead}"),
                Color::Black,
                Color::LightBlue,
            ));
            first = false;
        }
        if remote_behind > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            spans.push(badge(
                format!("r-{remote_behind}"),
                Color::Black,
                Color::LightYellow,
            ));
            first = false;
        }
        if conflicts > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            spans.push(badge(format!("!{conflicts}"), Color::White, Color::Red));
            first = false;
        }
        if let Some((n, status)) = pr_open {
            if !first {
                spans.push(Span::raw(" "));
            }
            let (fg, bg) = pr_badge_style(status);
            spans.push(badge(format!("PR#{n}"), fg, bg));
            first = false;
        }
        if let Some((_, span)) = stack_badge {
            if !first {
                spans.push(Span::raw(" "));
            }
            spans.push(span);
            first = false;
        }

        if !first {
            spans.push(Span::raw("  "));
        }
        for (idx, b) in buttons.iter().enumerate() {
            spans.push(Span::styled(b.label.clone(), b.style));
            if idx + 1 < buttons.len() {
                spans.push(Span::raw(" "));
            }
        }
    }

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Repo")
            .border_style(border_style),
    );
    f.render_widget(p, area);
}

fn action_for_key(code: KeyCode) -> Option<DiffRepoAction> {
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

impl DiffRepoBar {
    pub(crate) fn render(f: &mut Frame, app: &AppState, area: Rect) {
        render_diff_repo_bar(f, app, area);
    }

    pub(crate) fn on_event(app: &mut AppState, event: DiffRepoBarEvent) -> bool {
        match event {
            DiffRepoBarEvent::Action(action) => {
                trigger_diff_repo_action(app, action);
                true
            }
            DiffRepoBarEvent::Key(key) => {
                let Some(action) = action_for_key(key.code) else {
                    return false;
                };
                trigger_diff_repo_action(app, action);
                true
            }
        }
    }
}

impl UiComponent for DiffRepoBar {
    type Event = DiffRepoBarEvent;

    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        DiffRepoBar::render(f, app, area);
    }

    fn hit_test(app: &AppState, area: Rect, col: u16, row: u16) -> Option<Self::Event> {
        diff_repo_bar_action_at(app, area, col, row).map(DiffRepoBarEvent::Action)
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        DiffRepoBar::on_event(app, event)
    }
}
