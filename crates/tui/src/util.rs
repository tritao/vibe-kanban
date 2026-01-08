use ratatui::text::Line;

use crate::state::{AppState, TaskStatus};

pub(crate) fn board_statuses(app: &AppState) -> Vec<TaskStatus> {
    if app.board.show_cancelled {
        vec![
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::InReview,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ]
    } else {
        vec![
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::InReview,
            TaskStatus::Done,
        ]
    }
}

pub(crate) fn line_plain_text(line: &Line<'_>) -> String {
    let mut out = String::new();
    for s in &line.spans {
        out.push_str(s.content.as_ref());
    }
    out
}

pub(crate) fn lines_plain_text(lines: &[Line<'_>]) -> String {
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&line_plain_text(line));
    }
    out
}

pub(crate) fn window_for_list(len: usize, selected: usize, height: usize) -> (usize, usize, usize) {
    if len == 0 || height == 0 {
        return (0, 0, 0);
    }

    if len <= height {
        return (0, len, selected.min(len - 1));
    }

    let selected = selected.min(len - 1);
    let mut start = selected.saturating_sub(height / 2);
    start = start.min(len.saturating_sub(height));
    let end = (start + height).min(len);
    (start, end, selected.saturating_sub(start))
}

pub(crate) fn canonicalize_path_lossy(path: &str) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| std::path::PathBuf::from(path))
        .to_string_lossy()
        .to_string()
}
