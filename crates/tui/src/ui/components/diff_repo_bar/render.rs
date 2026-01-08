use std::time::Instant;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::{badges, buttons, shared};
use crate::{
    state::{AppState, FocusPane},
    store::git_status::RepoStatusRef,
    text::{display_width, truncate_to_width},
    ui::button_row::{button_row_plain, push_button_row_spans},
};

fn badge(text: impl Into<String>, fg: Color, bg: Color) -> Span<'static> {
    crate::ui::widgets::badge(text, fg, bg)
}

pub(super) fn render_diff_repo_bar(f: &mut Frame, app: &AppState, area: Rect) {
    let border_style = if app.ui.focus == FocusPane::Diff {
        crate::ui::palette::border_active()
    } else {
        Style::default()
    };

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
    ) = if let Some(r) = repo {
        let r = RepoStatusRef::new(r);
        let ahead = r.commits_ahead();
        let behind = r.commits_behind();
        let remote_ahead = r.remote_commits_ahead();
        let remote_behind = r.remote_commits_behind();
        let dirty_count = r.uncommitted_count();
        let untracked = r.untracked_count();
        let conflicts = r.conflicts_count();
        let pr_open = r.pr_badge();
        (
            r.repo_name().to_string(),
            r.target_branch_name().to_string(),
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
    let buttons = buttons::repo_bar_button_specs(app, repo, now);
    let stack_badge = badges::selected_stack_badge(app, repo);

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
    right_plain.push_str(&button_row_plain(&buttons));
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
            let (fg, bg) = crate::ui::palette::diff_repo_bar_dirty_badge();
            spans.push(badge(format!("Δ{dirty}"), fg, bg));
            first = false;
        }
        if untracked > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            let (fg, bg) = crate::ui::palette::diff_repo_bar_untracked_badge();
            spans.push(badge(format!("?{untracked}"), fg, bg));
            first = false;
        }
        if ahead > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            let (fg, bg) = crate::ui::palette::diff_repo_bar_ahead_badge();
            spans.push(badge(format!("+{ahead}"), fg, bg));
            first = false;
        }
        if behind > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            let (fg, bg) = crate::ui::palette::diff_repo_bar_behind_badge();
            spans.push(badge(format!("{behind}"), fg, bg));
            first = false;
        }
        if remote_ahead > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            let (fg, bg) = crate::ui::palette::diff_repo_bar_remote_ahead_badge();
            spans.push(badge(format!("r+{remote_ahead}"), fg, bg));
            first = false;
        }
        if remote_behind > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            let (fg, bg) = crate::ui::palette::diff_repo_bar_remote_behind_badge();
            spans.push(badge(format!("r-{remote_behind}"), fg, bg));
            first = false;
        }
        if conflicts > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            let (fg, bg) = crate::ui::palette::diff_repo_bar_conflicts_badge();
            spans.push(badge(format!("!{conflicts}"), fg, bg));
            first = false;
        }
        if let Some((n, status)) = pr_open {
            if !first {
                spans.push(Span::raw(" "));
            }
            let (fg, bg) = badges::pr_badge_style(status);
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
        push_button_row_spans(&mut spans, &buttons);
    }

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Repo")
            .border_style(border_style),
    );
    f.render_widget(p, area);
}
