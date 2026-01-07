use ratatui::text::Line;
use uuid::Uuid;

use crate::{
    commands::{run_latest_job, run_net_job},
    events::NetEvent,
    net::ops::{commit_list_http, commit_show_http},
    state::{AppState, CommitEntry, DiffListMode, JobKey, repo_scope::selected_repo_id},
};

const COMMIT_FILES_MARKER: &str = "----8<---- VK-FILES ----8<----";
const COMMIT_LIST_PAGE_SIZE: usize = 80;

fn sanitize_for_terminal(s: &str) -> String {
    // Tabs cause cursor jumps in terminals but are treated as a single cell in ratatui buffers,
    // which can leave visual artifacts when switching between lines of different lengths.
    // `git show --name-status` uses tabs between status and path, so we expand them.
    s.replace('\t', "    ").replace('\r', "")
}

pub(crate) fn sanitize_commit_preview_text(text: &str) -> String {
    // Keep this in sync with how we render the preview in the UI.
    // - Expand tabs (git name-status uses tabs).
    // - Strip CRs to avoid CRLF cursor oddities.
    sanitize_for_terminal(text)
}

pub(crate) fn ensure_commit_preview_rendered(app: &mut AppState, preview_width: u16) -> bool {
    if app.diff.list_mode != DiffListMode::Commits {
        return false;
    }
    let preview_width = preview_width.max(1);
    if app.diff.commit_preview_render_width == preview_width {
        return false;
    }
    let Some(text) = app.diff.commit_preview_text.clone() else {
        return false;
    };

    let mut out: Vec<Line<'static>> = vec![];
    let mut it = text.lines();

    // Header (plain, styled): commit / Author / Date until first blank line.
    while let Some(raw) = it.next() {
        if raw.trim().is_empty() {
            break;
        }
        let line = raw.to_string();
        let styled = if line.starts_with("commit ") {
            Line::styled(
                line,
                ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            )
        } else if line.starts_with("Author:") || line.starts_with("Date:") {
            Line::styled(
                line,
                ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::DIM),
            )
        } else {
            Line::from(line)
        };
        out.push(styled);
    }

    let mut message = String::new();
    let mut files = String::new();
    let mut in_files = false;
    for raw in it {
        if raw.trim_end() == COMMIT_FILES_MARKER {
            in_files = true;
            continue;
        }
        if in_files {
            files.push_str(raw);
            files.push('\n');
        } else {
            message.push_str(raw);
            message.push('\n');
        }
    }

    let content_width = preview_width.saturating_sub(2) as usize;
    if !message.trim().is_empty() {
        // We intentionally remove a single 4-space indent from all commit message lines.
        // Many tools format commit bodies with indentation, but in Markdown a leading 4 spaces
        // turns content into an indented code block (making `**bold**` render literally).
        // If you need code formatting in commit messages, use fenced blocks instead.
        let message = message
            .lines()
            .map(|line| line.strip_prefix("    ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");

        out.push(Line::from(""));
        out.extend(crate::md::render(
            message.trim_end(),
            content_width.max(1),
            crate::md::MdSoftBreakMode::Newline,
        ));
    }
    if !files.trim().is_empty() {
        let md = format!("\n\n```text\n{}\n```\n", files.trim_end());
        out.extend(crate::md::render(
            &md,
            content_width.max(1),
            crate::md::MdSoftBreakMode::Newline,
        ));
    }

    if out.is_empty() {
        out.push(Line::from("No commit selected"));
    }
    app.diff.commit_preview_lines = out;
    app.diff.commit_preview_render_width = preview_width;
    true
}

pub(crate) fn request_commit_list_refresh(app: &mut AppState) {
    if app.diff.list_mode != DiffListMode::Commits {
        return;
    }
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };
    let Some(repo_id) = selected_repo_id(app) else {
        return;
    };
    crate::ui::loading::start_with_delay(
        app.diff.commits_loading_by_repo.entry(repo_id).or_default(),
        crate::ui::constants::COMMIT_LIST_LOADING_INDICATOR_DELAY,
    );

    run_net_job(
        app,
        JobKey::CommitList,
        move |base_url, net_tx| async move {
            let limit = COMMIT_LIST_PAGE_SIZE;
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
        },
    );
}

pub(crate) fn request_commit_list_more(app: &mut AppState) {
    if app.diff.list_mode != DiffListMode::Commits {
        return;
    }
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };
    let Some(repo_id) = selected_repo_id(app) else {
        return;
    };
    if crate::jobs::job_running(app, JobKey::CommitList) {
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
    crate::ui::loading::start_with_delay(
        app.diff.commits_loading_by_repo.entry(repo_id).or_default(),
        crate::ui::constants::COMMIT_LIST_LOADING_INDICATOR_DELAY,
    );

    run_net_job(
        app,
        JobKey::CommitList,
        move |base_url, net_tx| async move {
            let limit = COMMIT_LIST_PAGE_SIZE;
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
        },
    );
}

pub(crate) fn request_commit_preview_refresh(app: &mut AppState) {
    if app.diff.list_mode != DiffListMode::Commits {
        return;
    }
    let Some(attempt_id) = app.board.selected_attempt_id else {
        return;
    };
    let Some(repo_id) = selected_repo_id(app) else {
        return;
    };
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

    crate::ui::loading::start_with_default_delay(&mut app.diff.commit_preview_loading, true);
    app.diff.commit_preview_text = None;
    app.diff.commit_preview_render_width = 0;
    run_latest_job(
        app,
        JobKey::CommitPreview,
        |app| &mut app.diff.commit_preview_gen,
        move |base_url, net_tx, generation| async move {
            match commit_show_http(&base_url, attempt_id, repo_id, &oid).await {
                Ok(text) => {
                    let _ = net_tx
                        .send(NetEvent::CommitPreviewLoaded {
                            repo_id,
                            text,
                            generation,
                        })
                        .await;
                }
                Err(e) => {
                    let message = format!("commit show failed: {e}");
                    let _ = net_tx.send(NetEvent::Error(message.clone())).await;
                    let _ = net_tx
                        .send(NetEvent::CommitPreviewFailed {
                            repo_id,
                            message,
                            generation,
                        })
                        .await;
                }
            }
        },
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
    app.diff
        .commits_loading_by_repo
        .entry(repo_id)
        .or_default()
        .stop();
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
