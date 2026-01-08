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

pub(crate) fn tick_with_delay(
    state: &mut LoadingState,
    now: Instant,
    is_running: bool,
    delay: std::time::Duration,
) -> bool {
    if state.delay != delay {
        state.delay = delay;
    }
    state.tick(now, is_running)
}
