use std::collections::HashSet;

use crate::selection::clamp_index;
use crate::state::{AppState, RepoBranchStatus};

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlagSpec {
    pub(crate) name: &'static str,
    pub(crate) desc: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SubcommandSpec {
    pub(crate) name: &'static str,
    pub(crate) desc: &'static str,
    pub(crate) flags: &'static [FlagSpec],
    pub(crate) help_syntax: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CommandSpec {
    pub(crate) name: &'static str,
    pub(crate) aliases: &'static [&'static str],
    pub(crate) desc: &'static str,
    pub(crate) flags: &'static [FlagSpec],
    pub(crate) subcommands: &'static [SubcommandSpec],
    pub(crate) help_syntax: &'static str,
    pub(crate) usage: Option<&'static str>,
}

const EMPTY_FLAGS: &[FlagSpec] = &[];
const EMPTY_SUBS: &[SubcommandSpec] = &[];

const REBASE_FLAGS: &[FlagSpec] = &[
    FlagSpec {
        name: "--onto",
        desc: "new base branch",
    },
    FlagSpec {
        name: "--old",
        desc: "old base branch",
    },
    FlagSpec {
        name: "--repo",
        desc: "repo name or index",
    },
];
const MERGE_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
}];
const PUSH_FLAGS: &[FlagSpec] = &[
    FlagSpec {
        name: "--force",
        desc: "force push",
    },
    FlagSpec {
        name: "--repo",
        desc: "repo name or index",
    },
];
const ABORT_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
}];
const RESOLVE_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
}];

const PR_CREATE_FLAGS: &[FlagSpec] = &[
    FlagSpec {
        name: "--title",
        desc: "PR title (required)",
    },
    FlagSpec {
        name: "--body",
        desc: "PR body",
    },
    FlagSpec {
        name: "--base",
        desc: "target branch",
    },
    FlagSpec {
        name: "--draft",
        desc: "create as draft",
    },
    FlagSpec {
        name: "--auto-desc",
        desc: "auto-generate description",
    },
    FlagSpec {
        name: "--repo",
        desc: "repo name or index",
    },
];
const PR_ATTACH_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
}];
const PR_COMMENTS_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
}];

const PR_SUBCOMMANDS: &[SubcommandSpec] = &[
    SubcommandSpec {
        name: "create",
        desc: "create a PR",
        flags: PR_CREATE_FLAGS,
        help_syntax: "/pr create --title T",
    },
    SubcommandSpec {
        name: "attach",
        desc: "attach existing PR",
        flags: PR_ATTACH_FLAGS,
        help_syntax: "/pr attach",
    },
    SubcommandSpec {
        name: "comments",
        desc: "fetch PR comments count",
        flags: PR_COMMENTS_FLAGS,
        help_syntax: "/pr comments",
    },
    SubcommandSpec {
        name: "open",
        desc: "open PR in browser",
        flags: EMPTY_FLAGS,
        help_syntax: "/pr open",
    },
];

const COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        name: "help",
        aliases: &["?"],
        desc: "show help",
        flags: EMPTY_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/help",
        usage: None,
    },
    CommandSpec {
        name: "status",
        aliases: &[],
        desc: "refresh branch status",
        flags: EMPTY_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/status",
        usage: None,
    },
    CommandSpec {
        name: "repo",
        aliases: &[],
        desc: "select repo for git ops",
        flags: EMPTY_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/repo [name|n]",
        usage: None,
    },
    CommandSpec {
        name: "rebase",
        aliases: &[],
        desc: "rebase attempt branch",
        flags: REBASE_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/rebase [--onto B]",
        usage: None,
    },
    CommandSpec {
        name: "resolve",
        aliases: &[],
        desc: "ask agent to resolve conflicts",
        flags: RESOLVE_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/resolve [--repo R]",
        usage: None,
    },
    CommandSpec {
        name: "abort",
        aliases: &[],
        desc: "abort conflicts/rebase",
        flags: ABORT_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/abort",
        usage: None,
    },
    CommandSpec {
        name: "merge",
        aliases: &[],
        desc: "squash-merge into target",
        flags: MERGE_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/merge",
        usage: None,
    },
    CommandSpec {
        name: "push",
        aliases: &[],
        desc: "push attempt branch",
        flags: PUSH_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/push [--force]",
        usage: None,
    },
    CommandSpec {
        name: "pr",
        aliases: &[],
        desc: "PR actions",
        flags: EMPTY_FLAGS,
        subcommands: PR_SUBCOMMANDS,
        help_syntax: "/pr <create|attach|comments|open>",
        usage: Some("usage: /pr <create|attach|comments|open>"),
    },
    CommandSpec {
        name: "open",
        aliases: &[],
        desc: "open file in editor",
        flags: EMPTY_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/open <file>",
        usage: Some("usage: /open <file_path>"),
    },
];

pub(crate) fn usage_for_command(cmd: &str) -> Option<&'static str> {
    find_command_spec(cmd).and_then(|c| c.usage)
}

pub(crate) fn help_section_lines() -> Vec<String> {
    let mut items: Vec<(&'static str, &'static str)> = vec![];
    for cmd in COMMAND_SPECS {
        if !cmd.subcommands.is_empty() {
            for sub in cmd.subcommands {
                items.push((sub.help_syntax, sub.desc));
            }
        } else {
            items.push((cmd.help_syntax, cmd.desc));
        }
    }

    let max_syntax = items.iter().map(|(s, _)| s.len()).max().unwrap_or(0);
    items
        .into_iter()
        .map(|(syntax, desc)| format!("  {syntax:<max_syntax$}  {desc}"))
        .collect()
}

pub(crate) fn unknown_command_error(cmd: &str) -> String {
    let needle = cmd.to_ascii_lowercase();
    let mut suggestions: Vec<&'static str> = vec![];
    for spec in COMMAND_SPECS {
        let name = spec.name;
        if name.starts_with(&needle)
            || spec
                .aliases
                .iter()
                .any(|a| a.to_ascii_lowercase().starts_with(&needle))
        {
            suggestions.push(name);
        }
    }

    match suggestions.as_slice() {
        [only] => format!("unknown command: /{cmd} (did you mean /{only}?)"),
        _ => format!("unknown command: /{cmd} (try /help)"),
    }
}

pub(crate) fn unknown_subcommand_error(cmd: &str, sub: &str) -> String {
    let Some(spec) = find_command_spec(cmd) else {
        return format!("unknown command: /{cmd} (try /help)");
    };
    let needle = sub.to_ascii_lowercase();
    let mut subs: Vec<&'static str> = spec
        .subcommands
        .iter()
        .map(|s| s.name)
        .filter(|s| s.starts_with(&needle))
        .collect();
    subs.sort_unstable();

    match subs.as_slice() {
        [only] => format!("unknown subcommand: {cmd} {sub} (did you mean {cmd} {only}?)"),
        _ => format!(
            "unknown subcommand: {cmd} {sub} (try {})",
            spec.help_syntax
        ),
    }
}

fn find_command_spec(token: &str) -> Option<&'static CommandSpec> {
    let t = token.to_ascii_lowercase();
    COMMAND_SPECS.iter().find(|c| {
        c.name == t || c.aliases.iter().any(|a| a.to_ascii_lowercase() == t)
    })
}

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

    let (tokens, current, ends_with_space) = split_for_completion(&app.ui.composer.buffer);

    let current_lower = current.to_ascii_lowercase();
    let used_flags: HashSet<&str> = tokens.iter().copied().filter(|t| t.starts_with("--")).collect();

    let mut out: Vec<CompletionItem> = vec![];

    fn push_flags(
        out: &mut Vec<CompletionItem>,
        flags: &[FlagSpec],
        used: &HashSet<&str>,
        current_lower: &str,
        current_is_empty: bool,
    ) {
        for flag in flags {
            if used.contains(flag.name) {
                continue;
            }
            if current_is_empty || flag.name.starts_with(current_lower) {
                out.push(CompletionItem {
                    insert: format!("{} ", flag.name),
                    desc: flag.desc.to_string(),
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
        for cmd in COMMAND_SPECS {
            let matches_name = cmd.name.starts_with(&current_lower);
            let matches_alias = cmd
                .aliases
                .iter()
                .any(|a| a.to_ascii_lowercase().starts_with(&current_lower));
            if current_is_empty || matches_name || matches_alias {
                out.push(CompletionItem {
                    insert: format!("{} ", cmd.name),
                    desc: cmd.desc.to_string(),
                });
            }
        }
        return out;
    }

    let Some(cmd_spec) = find_command_spec(tokens[0]) else {
        let needle = tokens[0].to_ascii_lowercase();
        for cmd in COMMAND_SPECS {
            let matches_name = cmd.name.starts_with(&needle);
            let matches_alias = cmd
                .aliases
                .iter()
                .any(|a| a.to_ascii_lowercase().starts_with(&needle));
            if matches_name || matches_alias {
                out.push(CompletionItem {
                    insert: format!("{} ", cmd.name),
                    desc: cmd.desc.to_string(),
                });
            }
        }
        return out;
    };

    match cmd_spec.name {
        "repo" => {
            push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
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
                    push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
                } else if current.starts_with("--") || ends_with_space {
                    push_flags(&mut out, sub.flags, &used_flags, &current_lower, current_is_empty);
                }
            }
        }
        "help" | "status" | "open" => {}
        _ => {
            if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(&mut out, &app.diff.repo_statuses, &current_lower, current_is_empty);
            } else if current.starts_with("--") || ends_with_space {
                push_flags(&mut out, cmd_spec.flags, &used_flags, &current_lower, current_is_empty);
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
