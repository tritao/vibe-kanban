use std::time::{Duration, Instant};

pub(crate) fn schedule_debounced(
    pending: &mut bool,
    next_at: &mut Option<Instant>,
    now: Instant,
    delay: Duration,
) {
    *pending = true;
    let next = now + delay;
    *next_at = match *next_at {
        Some(existing) => Some(existing.max(next)),
        None => Some(next),
    };
}

pub(crate) fn schedule_soonest(
    pending: &mut bool,
    next_at: &mut Option<Instant>,
    now: Instant,
    delay: Duration,
) {
    *pending = true;
    let next = now + delay;
    *next_at = match *next_at {
        Some(existing) => Some(existing.min(next)),
        None => Some(next),
    };
}

pub(crate) fn debounced_ready(pending: bool, next_at: Option<Instant>, now: Instant) -> bool {
    pending && next_at.map(|t| now >= t).unwrap_or(true)
}

pub(crate) fn clear_debounced(pending: &mut bool, next_at: &mut Option<Instant>) {
    *pending = false;
    *next_at = None;
}
