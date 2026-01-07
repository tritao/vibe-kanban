use std::time::Instant;

use crate::state::{DelayedLoadingIndicator, LoadingState};

pub(crate) fn start_with_default_delay(state: &mut LoadingState, placeholder_pending: bool) {
    state.start(
        Instant::now(),
        crate::ui::constants::LOADING_INDICATOR_DELAY,
        placeholder_pending,
    );
}

pub(crate) fn start_indicator_with_delay(
    indicator: &mut DelayedLoadingIndicator,
    delay: std::time::Duration,
) {
    indicator.start(Instant::now(), delay);
}
