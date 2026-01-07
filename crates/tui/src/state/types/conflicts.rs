use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConflictOp {
    Rebase,
    Merge,
    CherryPick,
    Revert,
}

pub(crate) fn display_conflict_op_label(op: Option<ConflictOp>) -> &'static str {
    match op {
        Some(ConflictOp::Merge) => "Merge",
        Some(ConflictOp::CherryPick) => "Cherry-pick",
        Some(ConflictOp::Revert) => "Revert",
        Some(ConflictOp::Rebase) | None => "Rebase",
    }
}

pub(crate) fn format_conflict_header(
    op: Option<ConflictOp>,
    source_branch: &str,
    base_branch: &str,
    repo_name: Option<&str>,
) -> String {
    let repo_context = repo_name
        .filter(|s| !s.trim().is_empty())
        .map(|r| format!(" in repository '{r}'"))
        .unwrap_or_default();
    match op {
        Some(ConflictOp::Merge) => {
            format!("Merge conflicts while merging into '{source_branch}'{repo_context}.")
        }
        Some(ConflictOp::CherryPick) => {
            format!("Cherry-pick conflicts on '{source_branch}'{repo_context}.")
        }
        Some(ConflictOp::Revert) => format!("Revert conflicts on '{source_branch}'{repo_context}."),
        Some(ConflictOp::Rebase) | None => format!(
            "Rebase conflicts while rebasing '{source_branch}' onto '{base_branch}'{repo_context}."
        ),
    }
}

pub(crate) fn build_resolve_conflicts_instructions(
    source_branch: Option<&str>,
    base_branch: Option<&str>,
    conflicted_files: &[String],
    op: Option<ConflictOp>,
    repo_name: Option<&str>,
) -> String {
    let source = source_branch
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("current attempt branch");
    let base = base_branch
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("base branch");

    let files_list: Vec<&String> = conflicted_files.iter().take(12).collect();
    let files_block = if files_list.is_empty() {
        String::new()
    } else {
        let mut out = String::from("\n\nFiles with conflicts:\n");
        for f in files_list {
            out.push_str("- ");
            out.push_str(f);
            out.push('\n');
        }
        out.pop(); // trailing '\n'
        out
    };

    let op_title = display_conflict_op_label(op);
    let header = format_conflict_header(op, source, base, repo_name);

    format!(
        "{header}{files_block}\n\nPlease resolve each file carefully. When continuing, ensure the {} does not hang (set `GIT_EDITOR=true` or use a non-interactive editor).",
        op_title.to_ascii_lowercase()
    )
}

#[cfg(test)]
mod conflict_instruction_tests {
    use super::*;

    #[test]
    fn builds_rebase_instructions_with_files() {
        let out = build_resolve_conflicts_instructions(
            Some("feat/x"),
            Some("main"),
            &vec!["a.txt".to_string(), "b.txt".to_string()],
            Some(ConflictOp::Rebase),
            Some("repo1"),
        );

        assert!(out.contains(
            "Rebase conflicts while rebasing 'feat/x' onto 'main' in repository 'repo1'."
        ));
        assert!(out.contains("Files with conflicts:\n- a.txt\n- b.txt"));
        assert!(out.contains("ensure the rebase does not hang"));
    }

    #[test]
    fn limits_conflicted_files_to_12() {
        let files: Vec<String> = (1..=20).map(|i| format!("file{i}.txt")).collect();
        let out = build_resolve_conflicts_instructions(
            Some("feat/x"),
            Some("main"),
            &files,
            Some(ConflictOp::Merge),
            Some("repo1"),
        );
        assert!(out.contains("- file12.txt"));
        assert!(!out.contains("- file13.txt"));
    }
}
