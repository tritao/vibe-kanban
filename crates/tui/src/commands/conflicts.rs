use crate::{
    layout::{compute_main_layout, current_terminal_rect},
    state::{AppState, FocusPane, build_resolve_conflicts_instructions},
    store::repo_status::RepoStatusRef,
    ui::toasts,
};

pub(crate) fn draft_conflict_resolution_for_repo_index(
    app: &mut AppState,
    repo_index: usize,
) -> bool {
    let Some(_attempt_id) = app.board.selected_attempt_id else {
        toasts::err_short(app, crate::ui::messages::errors::NO_ATTEMPT_SELECTED);
        return false;
    };
    if repo_index >= app.diff.repo_statuses.len() {
        return false;
    }

    crate::selection::change::set_selected_repo_index(app, repo_index);
    let repo = app.diff.repo_statuses.get(repo_index).expect("repo index");
    let repo_ref = RepoStatusRef::new(repo);

    let attempt_branch = app
        .board
        .selected_attempt_id
        .and_then(|id| app.board.attempts.iter().find(|a| a.id == id))
        .map(|a| a.branch.clone())
        .unwrap_or_else(|| "—".to_string());

    let instructions = build_resolve_conflicts_instructions(
        Some(&attempt_branch),
        Some(repo_ref.target_branch_name()),
        repo_ref.conflicted_files(),
        repo_ref.conflict_op(),
        Some(repo_ref.repo_name()),
    );

    app.ui.focus_execution();
    app.ui.composer_active = true;
    app.ui.composer_suggest_index = 0;
    app.ui.refresh_branch_status_after_send = true;
    app.ui.composer.buffer = instructions;
    app.ui.composer.set_end();

    let layout = compute_main_layout(current_terminal_rect(), FocusPane::Execution);
    let area = layout.exec_input;
    let (_inner_w, inner_h) = crate::ui::geometry::inner_size(area);
    let prefix_w = crate::text::display_width("  ");
    let content_w = crate::ui::geometry::inner_content_width(area, prefix_w, 1);
    app.ui
        .composer
        .ensure_cursor_visible(content_w, inner_h.max(1));

    toasts::info_short(
        app,
        format!("Conflicts: drafted resolution request ({})", repo.repo_name),
    );
    true
}
