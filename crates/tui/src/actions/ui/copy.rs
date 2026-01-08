use std::time::Instant;

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
            app.exec.log_mouse_select_range.map(|(a, b)| {
                let a = a.min(b);
                let b = b.max(a);
                crate::util::lines_plain_text(app.exec.log_lines.get(a..=b).unwrap_or(&[]))
            })
        }
        .unwrap_or_else(|| {
            let layout = compute_main_layout(current_terminal_rect(), app.ui.focus);
            let area = layout.exec_logs;
            let len = app.exec.log_lines.len();
            let max_render = area.height.saturating_sub(2) as usize;
            let visible = max_render.min(len).max(1);
            let (start, end) = app.exec.log_scroll.visible_range(len, visible);
            crate::util::lines_plain_text(app.exec.log_lines.get(start..end).unwrap_or(&[]))
        }),
        CopyTarget::DiffFiles => {
            let rows = crate::store::diff::DiffStore::new(&app.diff.diff_store)
                .rows_with_all_filtered(app.diff.diff_show_untracked);
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
            color: crate::ui::palette::toast_warn(),
            expires_at: Some(Instant::now() + crate::ui::constants::TOAST_SHORT),
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
            color: crate::ui::palette::toast_ok(),
            expires_at: Some(Instant::now() + crate::ui::constants::TOAST_SHORT),
        },
    ]
}
