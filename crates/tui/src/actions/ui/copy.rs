use std::time::Instant;

use ratatui::style::Color;

use super::{CopyTarget, Effect};
use crate::{
    layout::{compute_main_layout, current_terminal_rect},
    state::AppState,
};

pub(super) fn reduce_copy(app: &mut AppState, target: CopyTarget) -> Vec<Effect> {
    let text = match target {
        CopyTarget::Execution => if let Some(sel) = app.exec.log_selected {
            app.exec
                .log_buffers
                .get(&sel.exec_id)
                .and_then(|b| b.rendered_entry_text(sel.entry_idx, app.exec.log_render_width))
        } else {
            None
        }
        .unwrap_or_else(|| {
            let layout = compute_main_layout(current_terminal_rect(), app.ui.focus);
            let area = layout.exec_logs;
            let len = app.exec.log_lines.len();
            let max_render = area.height.saturating_sub(2) as usize;
            let visible = max_render.min(len).max(1);
            let mut offset = if app.exec.log_autoscroll {
                0
            } else {
                app.exec.log_scroll_offset
            };
            offset = offset.min(len.saturating_sub(visible));
            let start = len.saturating_sub(visible + offset);
            let end = len.saturating_sub(offset);
            crate::util::lines_plain_text(app.exec.log_lines.get(start..end).unwrap_or(&[]))
        }),
        CopyTarget::DiffFiles => {
            let rows = crate::diff::diff_rows_with_all_filtered(
                &app.diff.diff_store,
                app.diff.diff_show_untracked,
            );
            rows.get(app.diff.selected_diff_index)
                .map(|d| d.key.clone())
                .unwrap_or_default()
        }
        CopyTarget::DiffPreview => crate::util::lines_plain_text(&app.diff.diff_preview_lines),
        CopyTarget::WorktreePath => app.ui.launch_repo_path.clone().unwrap_or_default(),
        CopyTarget::AttemptCheckoutPath => app
            .diff
            .repo_statuses
            .get(app.diff.selected_repo_index)
            .and_then(|r| r.worktree_path.clone())
            .unwrap_or_default(),
    };

    if text.trim().is_empty() {
        return vec![Effect::Toast {
            message: "Copy: nothing to copy".to_string(),
            color: Color::Yellow,
            expires_at: Some(Instant::now() + std::time::Duration::from_secs(2)),
        }];
    }

    let label = match target {
        CopyTarget::Execution => "Copied logs",
        CopyTarget::DiffFiles => "Copied path",
        CopyTarget::DiffPreview => "Copied diff",
        CopyTarget::WorktreePath => "Copied worktree path",
        CopyTarget::AttemptCheckoutPath => "Copied attempt checkout path",
    };

    vec![
        Effect::CopyOsc52(text),
        Effect::Toast {
            message: label.to_string(),
            color: Color::Green,
            expires_at: Some(Instant::now() + std::time::Duration::from_secs(2)),
        },
    ]
}
