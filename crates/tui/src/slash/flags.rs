use std::collections::HashMap;

use super::types::{FlagSpec, ParsedFlags};

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

pub(crate) fn parse_flags_mixed(
    tokens: &[String],
    start_index: usize,
    flags: &'static [FlagSpec],
    help_hint: &'static str,
) -> Result<(ParsedFlags, Vec<String>), String> {
    let mut lookup: HashMap<&str, &FlagSpec> = HashMap::with_capacity(flags.len());
    for flag in flags {
        lookup.insert(flag.name, flag);
    }

    let mut parsed = ParsedFlags::default();
    let mut rest: Vec<String> = vec![];

    let mut i = start_index;
    while i < tokens.len() {
        let token = tokens[i].as_str();
        if !token.starts_with("--") {
            rest.push(tokens[i].clone());
            i += 1;
            continue;
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
            if parsed.values.contains_key(spec.name) || parsed.bools.contains(spec.name) {
                return Err(format!("duplicate flag: {} (try {help_hint})", spec.name));
            }
            parsed.values.insert(spec.name, value.clone());
        } else {
            if parsed.values.contains_key(spec.name) || parsed.bools.contains(spec.name) {
                return Err(format!("duplicate flag: {} (try {help_hint})", spec.name));
            }
            parsed.bools.insert(spec.name);
        }
        i += 1;
    }

    Ok((parsed, rest))
}
