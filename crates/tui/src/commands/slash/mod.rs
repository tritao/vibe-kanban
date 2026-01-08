use super::context::{
    require_repo_status_loaded, require_selected_attempt_id, resolve_repo_for_command,
};
use crate::{
    state::AppState,
    ui::{DiffRepoAction, trigger_diff_repo_action},
};

mod commits;
mod delete;
mod open;
mod repo;
mod stack;

mod submit;
pub(crate) use submit::submit_composer;

mod git;
pub(crate) use git::trigger_abort_conflicts;

mod executor;
mod model;
mod pr;

fn parse_slash_command(app: &mut AppState, tokens: &[String]) -> Result<bool, String> {
    let cmd =
        crate::slash::canonical_command_name(tokens[0].as_str()).unwrap_or(tokens[0].as_str());
    match cmd {
        "help" => {
            app.ui.show_help = true;
            app.ui.set_notice(crate::ui::messages::notices::HELP_OPENED);
            Ok(false)
        }
        "quit" => Ok(true),
        "status" => {
            trigger_diff_repo_action(app, DiffRepoAction::RefreshStatus);
            Ok(false)
        }
        "commits" => {
            commits::handle_commits_command(app)?;
            Ok(false)
        }
        "files" => {
            crate::commands::select_files_mode(app);
            crate::diff_preview::schedule_diff_preview_refresh(
                app,
                crate::ui::constants::DIFF_PREVIEW_REFRESH_DELAY,
            );
            Ok(false)
        }
        "stack" => {
            stack::handle_stack_command(app, tokens)?;
            Ok(false)
        }
        "resolve" => {
            git::handle_resolve_command(app, tokens)?;
            Ok(false)
        }
        "repo" => {
            repo::handle_repo_command(app, tokens.get(1).map(|s| s.as_str()))?;
            Ok(false)
        }
        "rebase" => {
            git::handle_rebase_command(app, tokens)?;
            Ok(false)
        }
        "abort" => {
            git::handle_abort_command(app, tokens)?;
            Ok(false)
        }
        "merge" => {
            git::handle_merge_command(app, tokens)?;
            Ok(false)
        }
        "push" => {
            git::handle_push_command(app, tokens)?;
            Ok(false)
        }
        "pr" => {
            pr::handle_pr_command(app, tokens)?;
            Ok(false)
        }
        "open" => {
            open::handle_open_command(app, tokens)?;
            Ok(false)
        }
        "executor" => {
            executor::handle_executor_command(app, tokens)?;
            Ok(false)
        }
        "model" => {
            model::handle_model_command(app, tokens)?;
            Ok(false)
        }
        "delete" => {
            delete::handle_delete_command(app, tokens)?;
            Ok(false)
        }
        _ => Err(crate::slash::unknown_command_error(tokens[0].as_str())),
    }
}
