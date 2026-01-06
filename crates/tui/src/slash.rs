use std::collections::{HashMap, HashSet};

use crate::{
    selection::clamp_index,
    state::{AppState, RepoBranchStatus},
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlagSpec {
    pub(crate) name: &'static str,
    pub(crate) desc: &'static str,
    pub(crate) takes_value: bool,
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
        takes_value: true,
    },
    FlagSpec {
        name: "--old",
        desc: "old base branch",
        takes_value: true,
    },
    FlagSpec {
        name: "--repo",
        desc: "repo name or index",
        takes_value: true,
    },
];
const MERGE_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
    takes_value: true,
}];
const PUSH_FLAGS: &[FlagSpec] = &[
    FlagSpec {
        name: "--force",
        desc: "force push",
        takes_value: false,
    },
    FlagSpec {
        name: "--repo",
        desc: "repo name or index",
        takes_value: true,
    },
];
const ABORT_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
    takes_value: true,
}];
const RESOLVE_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
    takes_value: true,
}];

const DELETE_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--subtree",
    desc: "delete subtree (destructive)",
    takes_value: false,
}];

const MODEL_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--effort",
    desc: "reasoning effort",
    takes_value: true,
}];

const PR_CREATE_FLAGS: &[FlagSpec] = &[
    FlagSpec {
        name: "--title",
        desc: "PR title (required)",
        takes_value: true,
    },
    FlagSpec {
        name: "--body",
        desc: "PR body",
        takes_value: true,
    },
    FlagSpec {
        name: "--base",
        desc: "target branch",
        takes_value: true,
    },
    FlagSpec {
        name: "--draft",
        desc: "create as draft",
        takes_value: false,
    },
    FlagSpec {
        name: "--auto-desc",
        desc: "auto-generate description",
        takes_value: false,
    },
    FlagSpec {
        name: "--repo",
        desc: "repo name or index",
        takes_value: true,
    },
];
const PR_ATTACH_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
    takes_value: true,
}];
const PR_COMMENTS_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
    takes_value: true,
}];
const PR_OPEN_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
    takes_value: true,
}];

const STACK_REPO_FLAGS: &[FlagSpec] = &[FlagSpec {
    name: "--repo",
    desc: "repo name or index",
    takes_value: true,
}];
const STACK_NEW_FLAGS: &[FlagSpec] = &[
    FlagSpec {
        name: "--name",
        desc: "patch name",
        takes_value: true,
    },
    FlagSpec {
        name: "--repo",
        desc: "repo name or index",
        takes_value: true,
    },
];
const STACK_DISABLE_FLAGS: &[FlagSpec] = &[
    FlagSpec {
        name: "--force",
        desc: "skip conflict guard (passes --force to stg cleanup)",
        takes_value: false,
    },
    FlagSpec {
        name: "--repo",
        desc: "repo name or index",
        takes_value: true,
    },
];
const STACK_REFRESH_FLAGS: &[FlagSpec] = &[
    FlagSpec {
        name: "--paths",
        desc: "comma-separated paths to refresh",
        takes_value: true,
    },
    FlagSpec {
        name: "--index",
        desc: "allow dirty index (stg refresh --index)",
        takes_value: false,
    },
    FlagSpec {
        name: "--repo",
        desc: "repo name or index",
        takes_value: true,
    },
];

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
        flags: PR_OPEN_FLAGS,
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
        name: "quit",
        aliases: &["exit"],
        desc: "exit the app",
        flags: EMPTY_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/quit",
        usage: Some("usage: /quit"),
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
        name: "files",
        aliases: &[],
        desc: "show file list in diff pane",
        flags: EMPTY_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/files",
        usage: None,
    },
    CommandSpec {
        name: "commits",
        aliases: &[],
        desc: "show commit list in diff pane",
        flags: EMPTY_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/commits",
        usage: None,
    },
    CommandSpec {
        name: "stack",
        aliases: &["stg"],
        desc: "stgit stack operations",
        flags: EMPTY_FLAGS,
        subcommands: &[
            SubcommandSpec {
                name: "status",
                desc: "refresh stack status",
                flags: STACK_REPO_FLAGS,
                help_syntax: "/stack status",
            },
            SubcommandSpec {
                name: "enable",
                desc: "enable stack mode (stg init)",
                flags: STACK_REPO_FLAGS,
                help_syntax: "/stack enable",
            },
            SubcommandSpec {
                name: "disable",
                desc: "disable stack mode (stg branch --cleanup)",
                flags: STACK_DISABLE_FLAGS,
                help_syntax: "/stack disable",
            },
            SubcommandSpec {
                name: "push",
                desc: "push next patch",
                flags: STACK_REPO_FLAGS,
                help_syntax: "/stack push",
            },
            SubcommandSpec {
                name: "pop",
                desc: "pop top patch",
                flags: STACK_REPO_FLAGS,
                help_syntax: "/stack pop",
            },
            SubcommandSpec {
                name: "new",
                desc: "create new patch",
                flags: STACK_NEW_FLAGS,
                help_syntax: "/stack new \"MSG\"",
            },
            SubcommandSpec {
                name: "refresh",
                desc: "refresh current patch",
                flags: STACK_REFRESH_FLAGS,
                help_syntax: "/stack refresh",
            },
            SubcommandSpec {
                name: "undo",
                desc: "undo last stack operation",
                flags: STACK_REPO_FLAGS,
                help_syntax: "/stack undo",
            },
            SubcommandSpec {
                name: "redo",
                desc: "redo last stack operation",
                flags: STACK_REPO_FLAGS,
                help_syntax: "/stack redo",
            },
        ],
        help_syntax: "/stack <...>",
        usage: Some(
            "usage: /stack status|enable|disable|new|refresh|push|pop|undo|redo [--repo R] ...",
        ),
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
    CommandSpec {
        name: "executor",
        aliases: &[],
        desc: "set default executor profile",
        flags: &[FlagSpec {
            name: "--variant",
            desc: "optional profile variant",
            takes_value: true,
        }],
        subcommands: EMPTY_SUBS,
        help_syntax: "/executor <name> [--variant V]",
        usage: Some("usage: /executor <name> [--variant V]"),
    },
    CommandSpec {
        name: "model",
        aliases: &[],
        desc: "set model / reasoning effort",
        flags: MODEL_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/model <MODEL> [--effort E]",
        usage: Some("usage: /model <MODEL> [--effort E]"),
    },
    CommandSpec {
        name: "delete",
        aliases: &["del", "rm"],
        desc: "delete selected task",
        flags: DELETE_FLAGS,
        subcommands: EMPTY_SUBS,
        help_syntax: "/delete [--subtree]",
        usage: Some("usage: /delete [--subtree]"),
    },
];

pub(crate) fn usage_for_command(cmd: &str) -> Option<&'static str> {
    find_command_spec(cmd).and_then(|c| c.usage)
}

pub(crate) fn canonical_command_name(cmd: &str) -> Option<&'static str> {
    find_command_spec(cmd).map(|c| c.name)
}

pub(crate) fn help_syntax_for_command(cmd: &str) -> Option<&'static str> {
    find_command_spec(cmd).map(|c| c.help_syntax)
}

pub(crate) fn flags_for_command(cmd: &str) -> &'static [FlagSpec] {
    find_command_spec(cmd)
        .map(|c| c.flags)
        .unwrap_or(EMPTY_FLAGS)
}

pub(crate) fn flags_for_subcommand(cmd: &str, sub: &str) -> Option<&'static [FlagSpec]> {
    let cmd = find_command_spec(cmd)?;
    let sub = cmd.subcommands.iter().find(|s| s.name == sub)?;
    Some(sub.flags)
}

pub(crate) fn help_syntax_for_subcommand(cmd: &str, sub: &str) -> Option<&'static str> {
    let cmd = find_command_spec(cmd)?;
    let sub = cmd.subcommands.iter().find(|s| s.name == sub)?;
    Some(sub.help_syntax)
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ParsedFlags {
    bools: HashSet<&'static str>,
    values: HashMap<&'static str, String>,
}

impl ParsedFlags {
    pub(crate) fn get_bool(&self, name: &str) -> bool {
        self.bools.contains(name)
    }

    pub(crate) fn get_value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(|s| s.as_str())
    }
}

pub(crate) fn parse_flags(
    tokens: &[String],
    start_index: usize,
    flags: &'static [FlagSpec],
    help_hint: &'static str,
) -> Result<ParsedFlags, String> {
    let mut lookup: HashMap<&str, &FlagSpec> = HashMap::with_capacity(flags.len());
    for flag in flags {
        lookup.insert(flag.name, flag);
    }

    let mut out = ParsedFlags::default();
    let mut i = start_index;
    while i < tokens.len() {
        let token = tokens[i].as_str();
        if !token.starts_with("--") {
            return Err(format!("unexpected arg: {token} (try {help_hint})"));
        }
        let Some(spec) = lookup.get(token).copied() else {
            return Err(format!("unknown flag: {token} (try {help_hint})"));
        };

        if spec.takes_value {
            i += 1;
            let value = tokens
                .get(i)
                .ok_or_else(|| format!("missing value for {} (try {help_hint})", spec.name))?;
            if value.starts_with("--") {
                return Err(format!("missing value for {} (try {help_hint})", spec.name));
            }
            if out.values.contains_key(spec.name) || out.bools.contains(spec.name) {
                return Err(format!("duplicate flag: {} (try {help_hint})", spec.name));
            }
            out.values.insert(spec.name, value.clone());
        } else {
            if out.values.contains_key(spec.name) || out.bools.contains(spec.name) {
                return Err(format!("duplicate flag: {} (try {help_hint})", spec.name));
            }
            out.bools.insert(spec.name);
        }
        i += 1;
    }

    Ok(out)
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
        _ => format!("unknown subcommand: {cmd} {sub} (try {})", spec.help_syntax),
    }
}

fn find_command_spec(token: &str) -> Option<&'static CommandSpec> {
    let t = token.to_ascii_lowercase();
    COMMAND_SPECS
        .iter()
        .find(|c| c.name == t || c.aliases.iter().any(|a| a.to_ascii_lowercase() == t))
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
    let used_flags: HashSet<&str> = tokens
        .iter()
        .copied()
        .filter(|t| t.starts_with("--"))
        .collect();

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
            if current_is_empty
                || name_l.starts_with(current_lower)
                || name_l.contains(current_lower)
            {
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
            let execs = app.ui.executor_profiles.as_object();

            let mut exec_key: Option<&str> = None;
            if let (Some(sel), Some(execs)) = (selection, execs) {
                exec_key = execs
                    .keys()
                    .find(|k| k.eq_ignore_ascii_case(&sel.executor))
                    .map(|s| s.as_str())
                    .or(Some(sel.executor.as_str()));
            }

            let wants_effort = tokens.last().is_some_and(|t| *t == "--effort")
                || tokens
                    .get(tokens.len().saturating_sub(2))
                    .is_some_and(|t| *t == "--effort");

            let allowed_efforts: &[&str] =
                match exec_key.unwrap_or("").to_ascii_lowercase().as_str() {
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
                if let (Some(sel), Some(execs)) = (selection, execs) {
                    let key = execs
                        .keys()
                        .find(|k| k.eq_ignore_ascii_case(&sel.executor))
                        .cloned()
                        .unwrap_or_else(|| sel.executor.clone());
                    if let Some(variants) = execs.get(&key).and_then(|v| v.as_object()) {
                        let mut models: HashSet<String> = HashSet::new();
                        for variant in variants.values() {
                            let Some(vobj) = variant.as_object() else {
                                continue;
                            };
                            let nested_key =
                                vobj.keys().find(|k| k.eq_ignore_ascii_case(&key)).cloned();
                            let Some(nested_key) = nested_key else {
                                continue;
                            };
                            let Some(cfg) = vobj.get(&nested_key).and_then(|v| v.as_object())
                            else {
                                continue;
                            };
                            if let Some(m) = cfg.get("model").and_then(|v| v.as_str()) {
                                if !m.trim().is_empty() {
                                    models.insert(m.to_string());
                                }
                            }
                        }
                        let mut models: Vec<String> = models.into_iter().collect();
                        models.sort();
                        for m in models {
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
                }
            }

            // Also allow picking effort directly without the intermediate `--effort` step.
            // We only show this when completing the first arg (so it doesn't spam in other positions).
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
