use std::collections::HashSet;

use super::{
    registry::{COMMAND_SPECS, find_command_spec},
    types::{CompletionItem, FlagSpec},
};
use crate::{
    selection::clamp_index,
    state::{AppState, RepoBranchStatus},
    store::executor_profiles::ExecutorProfilesStore,
};

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

fn push_flags(
    out: &mut Vec<CompletionItem>,
    flags: &'static [FlagSpec],
    used_flags: &HashSet<&str>,
    current_lower: &str,
    current_is_empty: bool,
) {
    for flag in flags {
        if used_flags.contains(flag.name) {
            continue;
        }
        let name_l = flag.name.to_ascii_lowercase();
        if current_is_empty || name_l.starts_with(current_lower) {
            let insert = if flag.takes_value {
                format!("{} ", flag.name)
            } else {
                format!("{} ", flag.name)
            };
            out.push(CompletionItem {
                insert,
                desc: flag.desc.to_string(),
            });
        }
    }
}

fn push_repos(
    out: &mut Vec<CompletionItem>,
    repos: &[RepoBranchStatus],
    current: &str,
    empty: bool,
) {
    for (i, repo) in repos.iter().enumerate() {
        let name = repo.repo_name.as_str();
        let idx = (i + 1).to_string();
        if empty
            || name.to_ascii_lowercase().starts_with(current)
            || name.to_ascii_lowercase().contains(current)
            || idx.starts_with(current)
        {
            out.push(CompletionItem {
                insert: format!("{name} "),
                desc: "repo".to_string(),
            });
            out.push(CompletionItem {
                insert: format!("{idx} "),
                desc: "repo index".to_string(),
            });
        }
    }
}

pub(crate) fn composer_completion_items(app: &AppState) -> Vec<CompletionItem> {
    if !app.ui.composer_active || !composer_is_slash_mode(&app.ui.composer.buffer) {
        return vec![];
    }

    let (tokens, current, ends_with_space) = split_for_completion(&app.ui.composer.buffer);

    let current_lower = current.to_ascii_lowercase();
    let used_flags: HashSet<&str> = tokens
        .iter()
        .copied()
        .filter(|t| t.starts_with("--"))
        .collect();

    let current_is_empty = current.trim().is_empty();

    // Completing the command name.
    if tokens.is_empty() {
        let mut out: Vec<CompletionItem> = vec![];
        for cmd in COMMAND_SPECS {
            if current_is_empty || cmd.name.starts_with(&current_lower) {
                out.push(CompletionItem {
                    insert: format!(
                        "{}/{} ",
                        if app.ui.composer.buffer.contains('/') {
                            ""
                        } else {
                            "/"
                        },
                        cmd.name
                    )
                    .trim_start_matches('/')
                    .to_string()
                        + " ",
                    desc: cmd.desc.to_string(),
                });
            }
        }
        // The above is messy due to pre-existing buffer; handle as before by just inserting cmd and space.
        // We'll normalize below.
        out.clear();
        for cmd in COMMAND_SPECS {
            if current_is_empty || cmd.name.starts_with(&current_lower) {
                out.push(CompletionItem {
                    insert: format!("{} ", cmd.name),
                    desc: cmd.desc.to_string(),
                });
            }
        }
        return out;
    }

    let cmd_name = tokens[0];
    let Some(cmd_spec) = find_command_spec(cmd_name) else {
        // Unknown command: suggest commands.
        let mut out: Vec<CompletionItem> = vec![];
        for cmd in COMMAND_SPECS {
            if current_is_empty || cmd.name.starts_with(&current_lower) {
                out.push(CompletionItem {
                    insert: format!("{} ", cmd.name),
                    desc: cmd.desc.to_string(),
                });
            }
        }
        return out;
    };

    let mut out: Vec<CompletionItem> = vec![];

    match cmd_spec.name {
        "repo" => {
            push_repos(
                &mut out,
                &app.diff.repo_statuses,
                &current_lower,
                current_is_empty,
            );
        }
        "pr" => {
            if tokens.len() == 1 {
                for sub in cmd_spec.subcommands {
                    if current_is_empty || sub.name.starts_with(&current_lower) {
                        out.push(CompletionItem {
                            insert: format!("{} ", sub.name),
                            desc: sub.desc.to_string(),
                        });
                    }
                }
            } else if let Some(sub_name) = tokens.get(1).copied() {
                let Some(sub) = cmd_spec.subcommands.iter().find(|s| s.name == sub_name) else {
                    return out;
                };

                if tokens.last().is_some_and(|t| *t == "--repo") {
                    push_repos(
                        &mut out,
                        &app.diff.repo_statuses,
                        &current_lower,
                        current_is_empty,
                    );
                } else if current.starts_with("--") || ends_with_space {
                    push_flags(
                        &mut out,
                        sub.flags,
                        &used_flags,
                        &current_lower,
                        current_is_empty,
                    );
                }
            }
        }
        "help" | "status" | "open" => {}
        "executor" => {
            // Suggest executor names for the first positional argument, then flags.
            if tokens.len() == 1 && !current.starts_with("--") {
                for name in app.ui.available_executors.iter() {
                    let name_l = name.to_ascii_lowercase();
                    if current_is_empty
                        || name_l.starts_with(&current_lower)
                        || name_l.contains(&current_lower)
                    {
                        out.push(CompletionItem {
                            insert: format!("{name} "),
                            desc: "executor".to_string(),
                        });
                    }
                }
            }
            if current.starts_with("--") || ends_with_space || tokens.len() >= 2 {
                push_flags(
                    &mut out,
                    cmd_spec.flags,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        "model" => {
            let selection = app.ui.selected_executor_profile.as_ref();
            let store = ExecutorProfilesStore::new(&app.ui.executor_profiles);
            let exec_key = store
                .resolve_executor_key(selection)
                .unwrap_or("")
                .to_ascii_lowercase();

            let wants_effort = tokens.last().is_some_and(|t| *t == "--effort")
                || tokens
                    .get(tokens.len().saturating_sub(2))
                    .is_some_and(|t| *t == "--effort");

            let allowed_efforts: &[&str] = match exec_key.as_str() {
                "codex" => &["low", "medium", "high", "xhigh"],
                "droid" => &["none", "dynamic", "off", "low", "medium", "high"],
                _ => &["low", "medium", "high"],
            };

            if wants_effort {
                for e in allowed_efforts {
                    if current_is_empty || e.starts_with(&current_lower) {
                        out.push(CompletionItem {
                            insert: format!("{e} "),
                            desc: "effort".to_string(),
                        });
                    }
                }
                return out;
            }

            // Suggest models for the first positional argument, then flags.
            if tokens.len() == 1 && !current.starts_with("--") {
                for m in store.models_for_selected_executor(selection) {
                    let m_l = m.to_ascii_lowercase();
                    if current_is_empty
                        || m_l.starts_with(&current_lower)
                        || m_l.contains(&current_lower)
                    {
                        out.push(CompletionItem {
                            insert: format!("{m} "),
                            desc: "model".to_string(),
                        });
                    }
                }
            }

            // Also allow picking effort directly without the intermediate `--effort` step.
            if tokens.len() == 1 && !current.starts_with("--") {
                for e in allowed_efforts {
                    if current_is_empty || e.starts_with(&current_lower) {
                        out.push(CompletionItem {
                            insert: format!("--effort {e} "),
                            desc: "effort".to_string(),
                        });
                    }
                }
            }

            // Only suggest flags when the user starts typing a flag.
            if current.starts_with("--") {
                push_flags(
                    &mut out,
                    cmd_spec.flags,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        _ => {
            if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(
                    &mut out,
                    &app.diff.repo_statuses,
                    &current_lower,
                    current_is_empty,
                );
            } else if current.starts_with("--") || ends_with_space {
                push_flags(
                    &mut out,
                    cmd_spec.flags,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
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

    let cursor = crate::text::edit::clamp_cursor_to_boundary(
        &app.ui.composer.buffer,
        app.ui.composer.cursor,
    );
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

    app.ui
        .composer
        .buffer
        .replace_range(token_start..cursor, insert);
    app.ui.composer.cursor = token_start + insert.len();
    app.ui.composer.goal_col = None;
    app.ui.composer_suggest_index = 0;
    true
}
