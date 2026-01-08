use std::time::Instant;

use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

use super::{DiffRepoAction, badges, buttons, shared};
use crate::{
    state::{AppState, RepoBranchStatus},
    store::repo_status::RepoStatusRef,
    text::{display_width, truncate_to_width},
    ui::button_row::{ButtonSpec, button_row_plain},
};

pub(super) struct DiffRepoBarLayout {
    pub(super) left: String,
    pub(super) can_show_right: bool,
    pub(super) badge_plains: Vec<String>,
    pub(super) badge_spans: Vec<Span<'static>>,
    pub(super) buttons: Vec<ButtonSpec<DiffRepoAction>>,
    pub(super) button_row_start_col: usize,
}

fn left_base(app: &AppState, repo: Option<&RepoBranchStatus>) -> String {
    let branch = shared::selected_attempt_branch(app);
    if let Some(r) = repo {
        let r = RepoStatusRef::new(r);
        format!("{}  {} → {}", r.repo_name(), branch, r.target_branch_name())
    } else if app.board.selected_attempt_id.is_some() {
        format!("{branch}  (press S for repo status)")
    } else {
        "(no attempt)".to_string()
    }
}

fn push_badge(
    badge_plains: &mut Vec<String>,
    badge_spans: &mut Vec<Span<'static>>,
    text: impl Into<String>,
    fg: ratatui::style::Color,
    bg: ratatui::style::Color,
) {
    let text = text.into();
    let plain = format!(" {text} ");
    badge_plains.push(plain.clone());
    badge_spans.push(Span::styled(
        plain,
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
    ));
}

pub(super) fn compute_repo_bar_layout(
    app: &AppState,
    inner_width: usize,
    now: Instant,
) -> DiffRepoBarLayout {
    let repo = shared::selected_repo_status(app);
    let repo_ref = repo.map(RepoStatusRef::new);

    let buttons = buttons::repo_bar_button_specs(app, repo, now);

    let mut badge_plains: Vec<String> = vec![];
    let mut badge_spans: Vec<Span<'static>> = vec![];

    if let Some(r) = repo_ref {
        let dirty = r.uncommitted_count();
        let untracked = r.untracked_count();
        let ahead = r.commits_ahead();
        let behind = r.commits_behind();
        let remote_ahead = r.remote_commits_ahead();
        let remote_behind = r.remote_commits_behind();
        let conflicts = r.conflicts_count();

        if dirty > 0 {
            let (fg, bg) = crate::ui::palette::diff_repo_bar_dirty_badge();
            push_badge(
                &mut badge_plains,
                &mut badge_spans,
                format!("Δ{dirty}"),
                fg,
                bg,
            );
        }
        if untracked > 0 {
            let (fg, bg) = crate::ui::palette::diff_repo_bar_untracked_badge();
            push_badge(
                &mut badge_plains,
                &mut badge_spans,
                format!("?{untracked}"),
                fg,
                bg,
            );
        }
        if ahead > 0 {
            let (fg, bg) = crate::ui::palette::diff_repo_bar_ahead_badge();
            push_badge(
                &mut badge_plains,
                &mut badge_spans,
                format!("+{ahead}"),
                fg,
                bg,
            );
        }
        if behind > 0 {
            let (fg, bg) = crate::ui::palette::diff_repo_bar_behind_badge();
            push_badge(
                &mut badge_plains,
                &mut badge_spans,
                format!("{behind}"),
                fg,
                bg,
            );
        }
        if remote_ahead > 0 {
            let (fg, bg) = crate::ui::palette::diff_repo_bar_remote_ahead_badge();
            push_badge(
                &mut badge_plains,
                &mut badge_spans,
                format!("r+{remote_ahead}"),
                fg,
                bg,
            );
        }
        if remote_behind > 0 {
            let (fg, bg) = crate::ui::palette::diff_repo_bar_remote_behind_badge();
            push_badge(
                &mut badge_plains,
                &mut badge_spans,
                format!("r-{remote_behind}"),
                fg,
                bg,
            );
        }
        if conflicts > 0 {
            let (fg, bg) = crate::ui::palette::diff_repo_bar_conflicts_badge();
            push_badge(
                &mut badge_plains,
                &mut badge_spans,
                format!("!{conflicts}"),
                fg,
                bg,
            );
        }

        if let Some((n, status)) = r.pr_badge() {
            let (fg, bg) = badges::pr_badge_style(status);
            push_badge(
                &mut badge_plains,
                &mut badge_spans,
                format!("PR#{n}"),
                fg,
                bg,
            );
        }

        if let Some((plain, span)) = badges::selected_stack_badge(app, repo) {
            badge_plains.push(plain);
            badge_spans.push(span);
        }
    }

    let mut right_plain = String::new();
    for (i, plain) in badge_plains.iter().enumerate() {
        if i > 0 {
            right_plain.push(' ');
        }
        right_plain.push_str(plain);
    }
    if !badge_plains.is_empty() {
        right_plain.push_str("  ");
    }
    right_plain.push_str(&button_row_plain(&buttons));
    let right_w = display_width(&right_plain);

    let can_show_right = inner_width > right_w + 2;

    let left_base = left_base(app, repo);
    let left = if can_show_right {
        let left_w = inner_width.saturating_sub(right_w).saturating_sub(2);
        truncate_to_width(&left_base, left_w)
    } else {
        truncate_to_width(&left_base, inner_width)
    };

    let button_row_start_col = if can_show_right {
        let mut cursor = display_width(&left);
        cursor = cursor.saturating_add(2); // after left "  "

        let mut first = true;
        for plain in &badge_plains {
            if !first {
                cursor = cursor.saturating_add(1);
            }
            first = false;
            cursor = cursor.saturating_add(display_width(plain));
        }
        if !badge_plains.is_empty() {
            cursor = cursor.saturating_add(2); // before buttons
        }
        cursor
    } else {
        0
    };

    DiffRepoBarLayout {
        left,
        can_show_right,
        badge_plains,
        badge_spans,
        buttons,
        button_row_start_col,
    }
}

pub(super) fn left_title_span(text: String) -> Span<'static> {
    Span::styled(text, Style::default().add_modifier(Modifier::BOLD))
}
