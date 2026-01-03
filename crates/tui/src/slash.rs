use std::collections::HashSet;

use crate::selection::clamp_index;
use crate::state::{AppState, RepoBranchStatus};

#[derive(Debug, Clone)]
pub(crate) struct CompletionItem {
    pub(crate) insert: String,
    pub(crate) desc: String,
}

pub(crate) fn composer_is_slash_mode(s: &str) -> bool {
    s.trim_start().starts_with('/')
}

fn split_for_completion(s: &str) -> (Vec<&str>, &str, bool) {
    let trimmed = s.trim_start();
    let Some(rest) = trimmed.strip_prefix('/') else {
        return (vec![], "", true);
    };
    let ends_with_space = rest.chars().last().is_some_and(|c| c.is_whitespace());
    let mut tokens: Vec<&str> = rest.split_whitespace().collect();
    if ends_with_space {
        return (tokens, "", true);
    }
    let current = tokens.pop().unwrap_or("");
    (tokens, current, false)
}

pub(crate) fn composer_completion_items(app: &AppState) -> Vec<CompletionItem> {
    if !app.ui.composer_active || !composer_is_slash_mode(&app.ui.composer.buffer) {
        return vec![];
    }

    const COMMANDS: &[(&str, &str)] = &[
        ("help", "show help"),
        ("status", "refresh branch status"),
        ("repo", "select repo for git ops"),
        ("rebase", "rebase attempt branch"),
        ("resolve", "ask agent to resolve conflicts"),
        ("abort", "abort conflicts/rebase"),
        ("merge", "squash-merge into target"),
        ("push", "push attempt branch"),
        ("pr", "PR actions"),
        ("open", "open file in editor"),
    ];

    const REBASE_FLAGS: &[(&str, &str)] = &[
        ("--onto", "new base branch"),
        ("--old", "old base branch"),
        ("--repo", "repo name or index"),
    ];
    const MERGE_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const PUSH_FLAGS: &[(&str, &str)] = &[("--force", "force push"), ("--repo", "repo name or index")];
    const ABORT_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const RESOLVE_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const PR_SUB: &[(&str, &str)] = &[
        ("create", "create a PR"),
        ("attach", "attach existing PR"),
        ("comments", "fetch PR comments count"),
        ("open", "open PR in browser"),
    ];
    const PR_CREATE_FLAGS: &[(&str, &str)] = &[
        ("--title", "PR title (required)"),
        ("--body", "PR body"),
        ("--base", "target branch"),
        ("--draft", "create as draft"),
        ("--auto-desc", "auto-generate description"),
        ("--repo", "repo name or index"),
    ];
    const PR_ATTACH_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const PR_COMMENTS_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];

    let (tokens, current, ends_with_space) = split_for_completion(&app.ui.composer.buffer);

    let current_lower = current.to_ascii_lowercase();
    let used_flags: HashSet<&str> = tokens.iter().copied().filter(|t| t.starts_with("--")).collect();

    let mut out: Vec<CompletionItem> = vec![];

    fn push_flags(
        out: &mut Vec<CompletionItem>,
        flags: &[(&'static str, &'static str)],
        used: &HashSet<&str>,
        current_lower: &str,
        current_is_empty: bool,
    ) {
        for (flag, desc) in flags {
            if used.contains(*flag) {
                continue;
            }
            if current_is_empty || flag.starts_with(current_lower) {
                out.push(CompletionItem {
                    insert: format!("{flag} "),
                    desc: desc.to_string(),
                });
            }
        }
    }

    fn push_repos(
        out: &mut Vec<CompletionItem>,
        repos: &[RepoBranchStatus],
        current_lower: &str,
        current_is_empty: bool,
    ) {
        for (idx, r) in repos.iter().enumerate() {
            let name = r.repo_name.as_str();
            let name_l = name.to_ascii_lowercase();
            if current_is_empty || name_l.starts_with(current_lower) || name_l.contains(current_lower) {
                out.push(CompletionItem {
                    insert: format!("{name} "),
                    desc: "repo".to_string(),
                });
            }
            let n = format!("{}", idx + 1);
            if current_is_empty || n.starts_with(current_lower) {
                out.push(CompletionItem {
                    insert: format!("{n} "),
                    desc: "repo index".to_string(),
                });
            }
        }
    }

    let current_is_empty = current.is_empty() || ends_with_space;

    if tokens.is_empty() {
        for (cmd, desc) in COMMANDS {
            if current_is_empty || cmd.starts_with(&current_lower) {
                out.push(CompletionItem {
                    insert: format!("{cmd} "),
                    desc: (*desc).to_string(),
                });
            }
        }
        return out;
    }

    match tokens[0] {
        "repo" => {
            push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
        }
        "rebase" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(&mut out, REBASE_FLAGS, &used_flags, &current_lower, current_is_empty);
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
            }
        }
        "merge" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(&mut out, MERGE_FLAGS, &used_flags, &current_lower, current_is_empty);
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
            }
        }
        "push" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(&mut out, PUSH_FLAGS, &used_flags, &current_lower, current_is_empty);
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
            }
        }
        "abort" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(&mut out, ABORT_FLAGS, &used_flags, &current_lower, current_is_empty);
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
            }
        }
        "resolve" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(&mut out, RESOLVE_FLAGS, &used_flags, &current_lower, current_is_empty);
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
            }
        }
        "pr" => {
            match tokens.get(1).copied() {
                Some("create") => {
                    if current.starts_with("--") || ends_with_space {
                        push_flags(
                            &mut out,
                            PR_CREATE_FLAGS,
                            &used_flags,
                            &current_lower,
                            current_is_empty,
                        );
                    } else if tokens.last().is_some_and(|t| *t == "--repo") {
                        push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
                    }
                }
                Some("attach") => {
                    if current.starts_with("--") || ends_with_space {
                        push_flags(
                            &mut out,
                            PR_ATTACH_FLAGS,
                            &used_flags,
                            &current_lower,
                            current_is_empty,
                        );
                    } else if tokens.last().is_some_and(|t| *t == "--repo") {
                        push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
                    }
                }
                Some("comments") => {
                    if current.starts_with("--") || ends_with_space {
                        push_flags(
                            &mut out,
                            PR_COMMENTS_FLAGS,
                            &used_flags,
                            &current_lower,
                            current_is_empty,
                        );
                    } else if tokens.last().is_some_and(|t| *t == "--repo") {
                        push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
                    }
                }
                Some("open") => {}
                Some(_) | None => {
                    for (sub, desc) in PR_SUB {
                        if current_is_empty || sub.starts_with(&current_lower) {
                            out.push(CompletionItem {
                                insert: format!("{sub} "),
                                desc: (*desc).to_string(),
                            });
                        }
                    }
                }
            }
        }
        "open" | "status" | "help" => {}
        _ => {}
    }

    out
}

pub(crate) fn move_composer_autocomplete(app: &mut AppState, delta: i32) {
    if !composer_is_slash_mode(&app.ui.composer.buffer) {
        return;
    }
    let items = composer_completion_items(app);
    if items.is_empty() {
        return;
    }
    let len = items.len();
    let cur = app.ui.composer_suggest_index.min(len - 1);
    let next = clamp_index(cur, delta, len);
    app.ui.composer_suggest_index = next;
}

pub(crate) fn apply_composer_autocomplete(app: &mut AppState) -> bool {
    if !composer_is_slash_mode(&app.ui.composer.buffer) {
        return false;
    }

    let items = composer_completion_items(app);
    if items.is_empty() {
        return false;
    }

    let idx = app.ui.composer_suggest_index.min(items.len() - 1);
    let insert = items[idx].insert.as_str();

    let cursor = crate::text::edit::clamp_cursor_to_boundary(&app.ui.composer.buffer, app.ui.composer.cursor);
    let buf_head = app.ui.composer.buffer.get(..cursor).unwrap_or("");
    let mut token_start = buf_head
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);

    if let Some(slash_pos) = buf_head.find('/') {
        if token_start <= slash_pos {
            token_start = slash_pos + 1;
        }
    }

    app.ui.composer.buffer.replace_range(token_start..cursor, insert);
    app.ui.composer.cursor = token_start + insert.len();
    app.ui.composer.goal_col = None;
    app.ui.composer_suggest_index = 0;
    true
}

