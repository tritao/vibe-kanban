use std::time::Instant;

use ratatui::layout::Rect;

use super::{DiffRepoAction, badges, buttons, shared};
use crate::{
    layout::rect_contains,
    state::AppState,
    store::repo_status::RepoStatusRef,
    text::{display_width, truncate_to_width},
    ui::button_row::{button_row_plain, hit_test_button_row},
};

pub(super) fn diff_repo_bar_action_at(
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
    let repo = shared::selected_repo_status(app);
    let branch = shared::selected_attempt_branch(app);

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
        let r = RepoStatusRef::new(r);
        let ahead = r.commits_ahead();
        let behind = r.commits_behind();
        let remote_ahead = r.remote_commits_ahead();
        let remote_behind = r.remote_commits_behind();
        let dirty = r.uncommitted_count();
        let untracked = r.untracked_count();
        let conflicts = r.conflicts_count();
        let pr_open = r.pr_number();
        let stack_badge_plain = badges::selected_stack_badge(app, repo).map(|(plain, _)| plain);
        (
            r.repo_name().to_string(),
            r.target_branch_name().to_string(),
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
    let buttons = buttons::repo_bar_button_specs(app, repo, now);

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
    right_plain.push_str(&button_row_plain(&buttons));
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

    hit_test_button_row(&buttons, inner_col, cursor)
}
