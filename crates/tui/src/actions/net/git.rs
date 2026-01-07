use crate::{
    commands::finish_git_op,
    events::GitOpKind,
    state::{AppState, StackStatusResponse},
};

pub(super) fn git_op_finished(
    app: &mut AppState,
    repo_id: Option<uuid::Uuid>,
    kind: GitOpKind,
    ok: bool,
    message: String,
) -> bool {
    finish_git_op(app, repo_id, kind, ok, message);
    if ok && app.diff.list_mode == crate::state::DiffListMode::Commits {
        let selected_repo_id = app
            .diff
            .repo_statuses
            .get(app.diff.selected_repo_index)
            .map(|r| r.repo_id);
        if repo_id.is_none() || repo_id == selected_repo_id {
            crate::commands::request_commit_list_refresh(app);
        }
    }
    true
}

pub(super) fn stack_status_loaded(
    app: &mut AppState,
    repo_id: uuid::Uuid,
    status: StackStatusResponse,
    generation: u64,
) -> bool {
    if !crate::async_jobs::is_latest_for(&app.diff.stack_status_gen_by_repo, repo_id, generation) {
        return true;
    }
    app.diff.stack_status_by_repo.insert(repo_id, status);
    true
}

pub(super) fn commit_list_loaded(
    app: &mut AppState,
    repo_id: uuid::Uuid,
    commits: Vec<crate::state::CommitEntry>,
    append: bool,
    has_more: bool,
) -> bool {
    crate::commands::apply_commit_list_page(app, repo_id, commits, append, has_more);
    true
}

pub(super) fn commit_preview_loaded(
    app: &mut AppState,
    repo_id: uuid::Uuid,
    text: String,
    generation: u64,
) -> bool {
    // Only update the preview if we're still looking at this repo.
    let selected_repo_id = app
        .diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .map(|r| r.repo_id);
    if selected_repo_id == Some(repo_id) && generation == app.diff.commit_preview_gen {
        app.diff.commit_preview_text = Some(crate::commands::sanitize_commit_preview_text(&text));
        app.diff.commit_preview_render_width = 0;
        app.diff.commit_preview_loading.stop();
    }
    true
}

pub(super) fn commit_preview_failed(
    app: &mut AppState,
    repo_id: uuid::Uuid,
    message: String,
    generation: u64,
) -> bool {
    let selected_repo_id = app
        .diff
        .repo_statuses
        .get(app.diff.selected_repo_index)
        .map(|r| r.repo_id);
    if selected_repo_id == Some(repo_id) && generation == app.diff.commit_preview_gen {
        app.diff.commit_preview_text = None;
        app.diff.commit_preview_lines = vec![ratatui::text::Line::from(message)];
        app.diff.commit_preview_render_width = 0;
        app.diff.commit_preview_loading.stop();
    }
    true
}

pub(super) fn commit_list_failed(app: &mut AppState, repo_id: uuid::Uuid) -> bool {
    app.diff
        .commits_loading_by_repo
        .entry(repo_id)
        .or_default()
        .stop();
    true
}
