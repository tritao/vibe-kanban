use ratatui::text::Line;
use uuid::Uuid;

use crate::{
    events::NetEvent,
    jobs::replace_job,
    net::ops::{commit_list_http, commit_show_http},
    state::{AppState, CommitEntry, DiffListMode, JobKey},
};

fn format_commit_show(text: &str) -> Vec<Line<'static>> {
    use ratatui::{
        style::{Modifier, Style},
        text::Span,
    };

    let mut out: Vec<Line<'static>> = vec![];
    for raw in text.lines() {
        let line = raw.to_string();
        if line.starts_with("commit ") {
            out.push(Line::from(Span::styled(
                line,
                Style::default().add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        if line.starts_with("Author:") || line.starts_with("Date:") {
            out.push(Line::from(Span::styled(
                line,
                Style::default().add_modifier(Modifier::DIM),
            )));
            continue;
        }
        out.push(Line::from(line));
    }
    out
}

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
    app.diff.commits_loading_by_repo.insert(repo_id, true);

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::CommitList,
        tokio::spawn(async move {
            let limit = 80usize;
            match commit_list_http(&base_url, attempt_id, repo_id, Some(limit), Some(0)).await {
                Ok(commits) => {
                    let has_more = commits.len() == limit;
                    let _ = net_tx
                        .send(NetEvent::CommitListLoaded {
                            repo_id,
                            commits,
                            append: false,
                            has_more,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("commit list failed: {e}")))
                        .await;
                    let _ = net_tx.send(NetEvent::CommitListFailed { repo_id }).await;
                }
            }
        }),
    );
}

pub(crate) fn request_commit_list_more(app: &mut AppState) {
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
    if app
        .diff
        .commits_loading_by_repo
        .get(&repo_id)
        .copied()
        .unwrap_or(false)
    {
        return;
    }
    if !app
        .diff
        .commits_has_more_by_repo
        .get(&repo_id)
        .copied()
        .unwrap_or(true)
    {
        return;
    }
    let offset = app
        .diff
        .commits_by_repo
        .get(&repo_id)
        .map(|v| v.len())
        .unwrap_or(0);
    app.diff.commits_loading_by_repo.insert(repo_id, true);

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::CommitList,
        tokio::spawn(async move {
            let limit = 80usize;
            match commit_list_http(&base_url, attempt_id, repo_id, Some(limit), Some(offset)).await
            {
                Ok(commits) => {
                    let has_more = commits.len() == limit;
                    let _ = net_tx
                        .send(NetEvent::CommitListLoaded {
                            repo_id,
                            commits,
                            append: true,
                            has_more,
                        })
                        .await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::Error(format!("commit list failed: {e}")))
                        .await;
                    let _ = net_tx.send(NetEvent::CommitListFailed { repo_id }).await;
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
    app.diff.commit_preview_lines = vec![Line::from("Loading commit…")];
    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    replace_job(
        app,
        JobKey::CommitPreview,
        tokio::spawn(async move {
            match commit_show_http(&base_url, attempt_id, repo_id, &oid).await {
                Ok(text) => {
                    let lines: Vec<Line<'static>> = format_commit_show(&text);
                    let _ = net_tx
                        .send(NetEvent::CommitPreviewLoaded { repo_id, lines })
                        .await;
                }
                Err(e) => {
                    let message = format!("commit show failed: {e}");
                    let _ = net_tx.send(NetEvent::Error(message.clone())).await;
                    let _ = net_tx
                        .send(NetEvent::CommitPreviewFailed { repo_id, message })
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

pub(crate) fn apply_commit_list_page(
    app: &mut AppState,
    repo_id: Uuid,
    commits: Vec<CommitEntry>,
    append: bool,
    has_more: bool,
) {
    app.diff.commits_loading_by_repo.insert(repo_id, false);
    app.diff.commits_has_more_by_repo.insert(repo_id, has_more);
    if append {
        app.diff
            .commits_by_repo
            .entry(repo_id)
            .or_default()
            .extend(commits);
    } else {
        app.diff.commits_by_repo.insert(repo_id, commits);
        app.diff.selected_commit_index = 0;
    }
    request_commit_preview_refresh(app);
}
