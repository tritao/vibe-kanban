use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Default, Clone)]
pub(crate) struct ParsedFlags {
    pub(super) bools: HashSet<&'static str>,
    pub(super) values: HashMap<&'static str, String>,
}

impl ParsedFlags {
    pub(crate) fn get_bool(&self, name: &str) -> bool {
        self.bools.contains(name)
    }

    pub(crate) fn get_value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CompletionItem {
    pub(crate) insert: String,
    pub(crate) desc: String,
}
