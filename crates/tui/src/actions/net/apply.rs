use crate::state::AppState;

#[derive(Default, Debug, Clone, Copy)]
pub(super) struct NetEffects {
    pub(super) sync_selected_repo: bool,
    pub(super) refresh_stack_status: bool,
    pub(super) refresh_commit_list: bool,
}

#[derive(Default, Debug, Clone, Copy)]
pub(super) struct NetApplyResult {
    pub(super) changed: bool,
    pub(super) effects: NetEffects,
}

impl NetApplyResult {
    pub(super) fn changed(changed: bool) -> Self {
        Self {
            changed,
            effects: NetEffects::default(),
        }
    }

    pub(super) fn with_effects(mut self, effects: NetEffects) -> Self {
        self.effects = effects;
        self
    }
}

pub(super) fn apply_effects(app: &mut AppState, effects: NetEffects) {
    if effects.sync_selected_repo {
        crate::ui::sync_selected_repo_from_diff_selection(app);
    }
    if effects.refresh_stack_status {
        crate::commands::request_stack_status_refresh(app);
    }
    if effects.refresh_commit_list {
        crate::commands::request_commit_list_refresh(app);
    }
}
