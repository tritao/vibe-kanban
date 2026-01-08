use super::{require_repo_status_loaded, require_selected_attempt_id, resolve_repo_for_command};
use crate::state::AppState;

pub(super) fn handle_stack_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let attempt_id = require_selected_attempt_id(app)?;
    require_repo_status_loaded(app)?;

    // Default: /stack status
    let sub = tokens.get(1).map(|s| s.as_str()).unwrap_or("status");
    match sub {
        "status" => {
            let help = crate::slash::help_syntax_for_subcommand("stack", "status")
                .unwrap_or("/stack status");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "status").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let _ = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::request_stack_status_refresh(app);
            app.ui.set_notice("Stack: refreshing…");
            Ok(())
        }
        "enable" => {
            let help = crate::slash::help_syntax_for_subcommand("stack", "enable")
                .unwrap_or("/stack enable");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "enable").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::trigger_stack_enable(app, attempt_id, repo_id);
            app.ui
                .set_notice(format!("Stack: enabling for {repo_name}…"));
            Ok(())
        }
        "disable" => {
            let help = crate::slash::help_syntax_for_subcommand("stack", "disable")
                .unwrap_or("/stack disable");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "disable").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            let force = parsed.get_bool("--force");
            crate::commands::trigger_stack_disable(app, attempt_id, repo_id, force);
            app.ui.set_notice(format!(
                "Stack: disabling for {repo_name}{}…",
                if force { " (force)" } else { "" }
            ));
            Ok(())
        }
        "new" => {
            let help =
                crate::slash::help_syntax_for_subcommand("stack", "new").unwrap_or("/stack new");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "new").unwrap_or(&[]),
                help,
            )?;
            let message = rest.join(" ").trim().to_string();
            if message.is_empty() {
                return Err(crate::slash::usage_for_command("stack")
                    .unwrap_or("usage: /stack new <MESSAGE> [--name N] [--repo R]")
                    .to_string());
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            let name = parsed.get_value("--name").map(ToString::to_string);
            crate::commands::trigger_stack_new(app, attempt_id, repo_id, name, message);
            app.ui.set_notice(format!("Stack: new ({repo_name})…"));
            Ok(())
        }
        "refresh" => {
            let help = crate::slash::help_syntax_for_subcommand("stack", "refresh")
                .unwrap_or("/stack refresh");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "refresh").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            let paths = parsed
                .get_value("--paths")
                .map(|s| {
                    s.split(',')
                        .map(|p| p.trim().to_string())
                        .filter(|p| !p.is_empty())
                        .collect::<Vec<_>>()
                })
                .filter(|v: &Vec<String>| !v.is_empty());
            let allow_dirty_index = parsed.get_bool("--index");
            crate::commands::trigger_stack_refresh(
                app,
                attempt_id,
                repo_id,
                paths,
                allow_dirty_index,
            );
            app.ui.set_notice(format!("Stack: refresh ({repo_name})…"));
            Ok(())
        }
        "push" => {
            let help =
                crate::slash::help_syntax_for_subcommand("stack", "push").unwrap_or("/stack push");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "push").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::trigger_stack_push(app, attempt_id, repo_id);
            app.ui.set_notice(format!("Stack: push ({repo_name})…"));
            Ok(())
        }
        "pop" => {
            let help =
                crate::slash::help_syntax_for_subcommand("stack", "pop").unwrap_or("/stack pop");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "pop").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::trigger_stack_pop(app, attempt_id, repo_id);
            app.ui.set_notice(format!("Stack: pop ({repo_name})…"));
            Ok(())
        }
        "undo" => {
            let help =
                crate::slash::help_syntax_for_subcommand("stack", "undo").unwrap_or("/stack undo");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "undo").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::trigger_stack_undo(app, attempt_id, repo_id);
            app.ui.set_notice(format!("Stack: undo ({repo_name})…"));
            Ok(())
        }
        "redo" => {
            let help =
                crate::slash::help_syntax_for_subcommand("stack", "redo").unwrap_or("/stack redo");
            let (parsed, rest) = crate::slash::parse_flags_mixed(
                tokens,
                2,
                crate::slash::flags_for_subcommand("stack", "redo").unwrap_or(&[]),
                help,
            )?;
            if !rest.is_empty() {
                return Err(format!("unexpected args: {} (try {help})", rest.join(" ")));
            }
            let (repo_id, repo_name) = resolve_repo_for_command(app, parsed.get_value("--repo"))?;
            crate::commands::trigger_stack_redo(app, attempt_id, repo_id);
            app.ui.set_notice(format!("Stack: redo ({repo_name})…"));
            Ok(())
        }
        other => Err(format!("unknown stack subcommand: {other}")),
    }
}
