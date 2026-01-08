use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::{events::StreamStatus, logs::types::ToolStatus, state::MergeStatus};

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

pub(crate) fn stream_status_style(status: StreamStatus) -> Style {
    Style::default().fg(stream_status_color(status))
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

pub(crate) fn chrome_hint() -> Style {
    Style::default().add_modifier(Modifier::DIM)
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

pub(crate) fn log_accent_user() -> Color {
    Color::Yellow
}

pub(crate) fn log_accent_assistant() -> Color {
    Color::Cyan
}

pub(crate) fn log_accent_system() -> Color {
    Color::Gray
}

pub(crate) fn log_accent_error() -> Color {
    Color::Red
}

pub(crate) fn log_accent_feedback() -> Color {
    Color::Yellow
}

pub(crate) fn log_tool_status_badge(status: ToolStatus) -> (Span<'static>, Style) {
    match status {
        ToolStatus::Success => (
            Span::styled("ok", Style::default().fg(Color::Green)),
            Style::default().fg(Color::Green),
        ),
        ToolStatus::Failed => (
            Span::styled("fail", Style::default().fg(Color::Red)),
            Style::default().fg(Color::Red),
        ),
        ToolStatus::Denied => (
            Span::styled("denied", Style::default().fg(Color::Yellow)),
            Style::default().fg(Color::Yellow),
        ),
        ToolStatus::PendingApproval => (
            Span::styled("approval", Style::default().fg(Color::Magenta)),
            Style::default().fg(Color::Magenta),
        ),
        ToolStatus::TimedOut => (
            Span::styled("timeout", Style::default().fg(Color::Yellow)),
            Style::default().fg(Color::Yellow),
        ),
        ToolStatus::Other => (
            Span::styled("…", Style::default().add_modifier(Modifier::DIM)),
            Style::default().add_modifier(Modifier::DIM),
        ),
    }
}

pub(crate) fn log_tool_kind(kind: &str) -> (&'static str, Color) {
    match kind {
        "file_read" | "search" => ("Explored", Color::Cyan),
        "file_edit" => ("Edited", Color::Green),
        "command_run" => ("Ran", Color::Cyan),
        "web_fetch" => ("Fetched", Color::Cyan),
        "task_create" => ("Created", Color::Green),
        "plan_presentation" => ("Plan", Color::Magenta),
        "todo_management" => ("Todos", Color::Magenta),
        "tool" => ("Tool", Color::Blue),
        _ => ("Tool", Color::Blue),
    }
}

pub(crate) fn log_markdown_prefix_fg() -> Color {
    Color::Gray
}

pub(crate) fn log_markdown_heading_fg() -> Color {
    Color::Cyan
}

pub(crate) fn log_markdown_inline_code_fg() -> Color {
    Color::Yellow
}

pub(crate) fn log_markdown_code_bg() -> Color {
    Color::DarkGray
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

pub(crate) fn diff_repo_action_color(action: crate::ui::components::DiffRepoAction) -> Color {
    use crate::ui::components::DiffRepoAction as A;
    match action {
        A::Merge => Color::Green,
        A::CreatePr | A::OpenPr => Color::Blue,
        A::Rebase => Color::Yellow,
        A::ResolveConflicts | A::RefreshStatus => Color::Cyan,
        A::OpenConflict => Color::Blue,
        A::AbortConflicts => Color::Red,
    }
}

pub(crate) fn diff_repo_action_button_style(
    action: crate::ui::components::DiffRepoAction,
    enabled: bool,
    recently_done_fail: bool,
) -> Style {
    let color = diff_repo_action_color(action);
    let mut style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    if !enabled {
        style = style.add_modifier(Modifier::DIM);
    }
    if recently_done_fail {
        style = Style::default().fg(error_fg()).add_modifier(Modifier::BOLD);
    }
    style
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

pub(crate) fn stack_badge_on() -> (Color, Color) {
    (Color::Black, Color::LightGreen)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn walk_rs_files(root: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_rs_files(&path, out);
                continue;
            }
            if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn no_hardcoded_fg_colors_outside_palette_and_diff_highlight() {
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let src_root = crate_root.join("src");

        let mut files = vec![];
        walk_rs_files(&src_root, &mut files);

        let banned = ".fg(Color::";
        let allowlist: [&str; 2] = ["src/ui/palette.rs", "src/diff/highlight.rs"];

        let mut offenders: Vec<String> = vec![];
        for file in files {
            let rel = file
                .strip_prefix(crate_root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|| file.to_string_lossy().replace('\\', "/"));

            if allowlist.iter().any(|a| rel.ends_with(a)) {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(&file) else {
                continue;
            };
            if content.contains(banned) {
                offenders.push(rel);
            }
        }

        assert!(
            offenders.is_empty(),
            "Found hardcoded .fg(Color::...) outside palette/diff highlight: {offenders:?}"
        );
    }
}
