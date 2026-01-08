use std::time::Duration;

pub(crate) const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(crate) const LOADING_INDICATOR_DELAY: Duration = Duration::from_millis(120);
pub(crate) const COMMIT_LIST_LOADING_INDICATOR_DELAY: Duration = Duration::from_millis(200);

pub(crate) const BRANCH_STATUS_AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(45);

pub(crate) const DIFF_ALL_DEBOUNCE_DELAY: Duration = Duration::from_millis(120);
pub(crate) const DIFF_PREVIEW_REFRESH_DELAY: Duration = Duration::from_millis(0);

pub(crate) const LOG_WHEEL_STEP: i32 = 3;
pub(crate) const DIFF_WHEEL_STEP: i32 = 3;

pub(crate) const TOAST_SHORT: Duration = Duration::from_secs(2);
pub(crate) const TOAST_MEDIUM: Duration = Duration::from_secs(3);
