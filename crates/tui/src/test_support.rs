use tokio::sync::{mpsc, watch};

use crate::{
    events::NetEvent,
    state::{AppState, LogMode},
};

pub(crate) fn mk_app() -> AppState {
    let (net_tx, _net_rx) = mpsc::channel::<NetEvent>(8);
    let (project_sel_tx, _project_sel_rx) = watch::channel(None);
    let (attempt_sel_tx, _attempt_sel_rx) = watch::channel(None);
    let (exec_sel_tx, _exec_sel_rx) = watch::channel(None);
    let (log_mode_tx, _log_mode_rx) = watch::channel(LogMode::Normalized);
    let (diff_stats_tx, _diff_stats_rx) = watch::channel(false);
    let (diff_reconnect_tx, _diff_reconnect_rx) = watch::channel(0u64);
    let (reconnect_tx, _reconnect_rx) = watch::channel(0u64);

    AppState::new(
        "http://127.0.0.1:1234".to_string(),
        net_tx,
        project_sel_tx,
        attempt_sel_tx,
        exec_sel_tx,
        log_mode_tx,
        diff_stats_tx,
        diff_reconnect_tx,
        reconnect_tx,
        crate::state::TuiPrefs::default(),
    )
}
