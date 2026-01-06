use ratatui::text::Line;
use uuid::Uuid;

use crate::{
    events::NetEvent,
    jobs::replace_job,
    net::ops::{commit_list_http, commit_show_http},
    state::{AppState, CommitEntry, DiffListMode, JobKey},
};

pub(crate) fn request_commit_list_refresh(app: &mut AppState) {
    if app.diff.list_mode != DiffListMode::Commits {
        return;
    }
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };
    let Some(repo) = app.diff.repo_statuses.get(app.diff.selected_repo_index) else {
        return;
    };
    let repo_id = repo.repo_id;

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::CommitList,
        tokio::spawn(async move {
            match commit_list_http(&base_url, attempt_id, repo_id, Some(80)).await {
                Ok(commits) => {
                    let _ = net_tx
                        .send(NetEvent::CommitListLoaded { repo_id, commits })
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("commit list failed: {e}")))
                        .await;
                }
            }
        }),
    );
}

pub(crate) fn request_commit_preview_refresh(app: &mut AppState) {
    if app.diff.list_mode != DiffListMode::Commits {
        return;
    }
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };
    let Some(repo) = app.diff.repo_statuses.get(app.diff.selected_repo_index) else {
        return;
    };
    let repo_id = repo.repo_id;
    let commits = app
        .diff
        .commits_by_repo
        .get(&repo_id)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    if commits.is_empty() {
        return;
    }
    let idx = app.diff.selected_commit_index.min(commits.len() - 1);
    let oid = commits[idx].oid.clone();

    app.diff.commit_preview_loading = true;
    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::CommitPreview,
        tokio::spawn(async move {
            match commit_show_http(&base_url, attempt_id, repo_id, &oid).await {
                Ok(text) => {
                    let lines: Vec<Line<'static>> =
                        text.lines().map(|l| Line::from(l.to_string())).collect();
                    let _ = net_tx
                        .send(NetEvent::CommitPreviewLoaded { repo_id, lines })
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("commit show failed: {e}")))
                        .await;
                }
            }
        }),
    );
}

pub(crate) fn select_commits_mode(app: &mut AppState) {
    app.diff.list_mode = DiffListMode::Commits;
    app.diff.selected_commit_index = 0;
    app.diff.diff_scroll_offset = 0;
    request_commit_list_refresh(app);
}

pub(crate) fn select_files_mode(app: &mut AppState) {
    app.diff.list_mode = DiffListMode::Files;
    app.diff.diff_scroll_offset = 0;
}

pub(crate) fn set_commit_list(app: &mut AppState, repo_id: Uuid, commits: Vec<CommitEntry>) {
    app.diff.commits_by_repo.insert(repo_id, commits);
    app.diff.selected_commit_index = 0;
    request_commit_preview_refresh(app);
}
