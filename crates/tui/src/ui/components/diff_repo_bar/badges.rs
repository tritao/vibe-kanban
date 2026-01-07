use ratatui::{style::Color, text::Span};

use crate::state::{AppState, MergeStatus, RepoBranchStatus};

fn badge(text: impl Into<String>, fg: Color, bg: Color) -> Span<'static> {
    crate::ui::widgets::badge(text, fg, bg)
}

pub(super) fn pr_badge_style(status: MergeStatus) -> (Color, Color) {
    crate::ui::palette::pr_merge_status_style(status)
}

pub(super) fn selected_stack_badge(
    app: &AppState,
    repo: Option<&RepoBranchStatus>,
) -> Option<(String, Span<'static>)> {
    let repo_id = repo?.repo_id;
    let status = app.diff.stack_status_by_repo.get(&repo_id)?;
    if !status.available {
        let (fg, bg) = crate::ui::palette::stack_badge_missing();
        return Some((
            " Stack: missing ".to_string(),
            badge("Stack: missing", fg, bg),
        ));
    }
    if !status.enabled {
        let (fg, bg) = crate::ui::palette::stack_badge_off();
        return Some((" Stack: off ".to_string(), badge("Stack: off", fg, bg)));
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
    let (fg, bg) = crate::ui::palette::stack_badge_on();
    Some((format!(" {label} "), badge(label, fg, bg)))
}
