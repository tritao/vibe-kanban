use crate::slash::types::{CommandSpec, FlagSpec, SubcommandSpec};

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

pub(super) const COMMAND_SPECS: &[CommandSpec] = &[
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

pub(super) fn find_command_spec(token: &str) -> Option<&'static CommandSpec> {
    let t = token.to_ascii_lowercase();
    COMMAND_SPECS
        .iter()
        .find(|c| c.name == t || c.aliases.iter().any(|a| a.to_ascii_lowercase() == t))
}

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
