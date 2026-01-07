use std::time::Instant;

use crate::state::LoadingState;

pub(crate) fn start_with_default_delay(state: &mut LoadingState, placeholder_pending: bool) {
    state.start(
        Instant::now(),
        crate::ui::constants::LOADING_INDICATOR_DELAY,
        placeholder_pending,
    );
}

pub(crate) fn start_with_delay(state: &mut LoadingState, delay: std::time::Duration) {
    state.start(Instant::now(), delay, false);
}
