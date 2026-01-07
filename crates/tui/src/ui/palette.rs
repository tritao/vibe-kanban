use ratatui::style::{Color, Modifier, Style};

use crate::{events::StreamStatus, state::MergeStatus};

pub(crate) fn border_active() -> Style {
    Style::default().fg(Color::Cyan)
}

pub(crate) fn border_inactive() -> Style {
    Style::default().fg(Color::Gray)
}

pub(crate) fn stream_status_color(status: StreamStatus) -> Color {
    match status {
        StreamStatus::Connecting => Color::Yellow,
        StreamStatus::Connected | StreamStatus::Completed => Color::Green,
        StreamStatus::Disconnected => Color::DarkGray,
        StreamStatus::Error => Color::Red,
    }
}

pub(crate) fn top_bar_project() -> Style {
    Style::default().fg(Color::Cyan)
}

pub(crate) fn top_bar_branch() -> Style {
    Style::default().fg(Color::Magenta)
}

pub(crate) fn chrome_meta() -> Style {
    Style::default().fg(Color::Gray)
}

pub(crate) fn toast_info() -> Color {
    Color::Cyan
}

pub(crate) fn toast_ok() -> Color {
    Color::Green
}

pub(crate) fn toast_warn() -> Color {
    Color::Yellow
}

pub(crate) fn toast_err() -> Color {
    Color::Red
}

pub(crate) fn error_fg() -> Color {
    Color::Red
}

pub(crate) fn notice_fg() -> Color {
    Color::Green
}

pub(crate) fn badge_fail() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(Color::Red)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn diff_label_all() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn diff_label_add() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn diff_label_del() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(Color::Red)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn diff_label_mod() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::LightBlue)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn diff_label_ren() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::LightYellow)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn diff_count_added() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn diff_count_deleted() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(Color::Red)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn diff_repo_bar_dirty_badge() -> (Color, Color) {
    (Color::Black, Color::LightYellow)
}

pub(crate) fn diff_repo_bar_untracked_badge() -> (Color, Color) {
    (Color::Black, Color::LightCyan)
}

pub(crate) fn diff_repo_bar_ahead_badge() -> (Color, Color) {
    (Color::Black, Color::LightGreen)
}

pub(crate) fn diff_repo_bar_behind_badge() -> (Color, Color) {
    (Color::Black, Color::LightYellow)
}

pub(crate) fn diff_repo_bar_remote_ahead_badge() -> (Color, Color) {
    (Color::Black, Color::LightBlue)
}

pub(crate) fn diff_repo_bar_remote_behind_badge() -> (Color, Color) {
    (Color::Black, Color::LightYellow)
}

pub(crate) fn diff_repo_bar_conflicts_badge() -> (Color, Color) {
    (Color::White, Color::Red)
}

pub(crate) fn diff_repo_action_color(
    action: crate::ui::components::diff_repo_bar::DiffRepoAction,
) -> Color {
    use crate::ui::components::diff_repo_bar::DiffRepoAction as A;
    match action {
        A::Merge => Color::Green,
        A::CreatePr | A::OpenPr => Color::Blue,
        A::Rebase => Color::Yellow,
        A::ResolveConflicts | A::RefreshStatus => Color::Cyan,
        A::OpenConflict => Color::Blue,
        A::AbortConflicts => Color::Red,
    }
}

pub(crate) fn pr_merge_status_style(status: MergeStatus) -> (Color, Color) {
    match status {
        MergeStatus::Open => (Color::Black, Color::LightBlue),
        MergeStatus::Merged => (Color::Black, Color::LightGreen),
        MergeStatus::Closed => (Color::White, Color::Red),
        MergeStatus::Unknown => (Color::Black, Color::LightYellow),
    }
}

pub(crate) fn stack_badge_missing() -> (Color, Color) {
    (Color::White, Color::Red)
}

pub(crate) fn stack_badge_off() -> (Color, Color) {
    (Color::Black, Color::LightYellow)
}

pub(crate) fn stack_badge_on() -> (Color, Color) {
    (Color::Black, Color::LightGreen)
}
