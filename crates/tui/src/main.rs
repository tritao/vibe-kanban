mod text;
mod diff;
mod logs;
mod ui;
mod layout;
mod render;
mod util;
mod net;
mod app;
mod fmt;
mod cli_parse;
mod events;
mod state;
mod selection;
mod diff_preview;
mod prefs;
mod commands;
mod actions;
mod jobs;
mod slash;

use std::time::Duration;

use clap::Parser;
use tokio::sync::mpsc;

use crate::events::{NetEvent, UiEvent};
use crate::net::ops::stop_exec_http;
use crate::state::{AppState, ConfirmAction};

#[derive(Parser, Debug, Clone)]
#[command(name = "vibe-kanban-tui")]
#[command(about = "Full-screen TUI dashboard for Vibe Kanban")]
struct Args {
    /// Backend base URL (e.g., http://127.0.0.1:3001)
    #[arg(long, env = "VIBE_BACKEND_URL")]
    backend_url: Option<String>,

    /// Backend host (used when backend-url not set)
    #[arg(long, env = "HOST")]
    host: Option<String>,

    /// Backend port (used when backend-url not set)
    #[arg(long, env = "BACKEND_PORT")]
    port: Option<u16>,

    /// Enable verbose logging
    #[arg(long, default_value_t = false)]
    verbose: bool,
}


// Core shared state/types live in `state` (import directly from `crate::state` in modules).

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}

// Selection/filter/navigation helpers live in `selection` (re-exported here for now).

fn spawn_input_reader(ui_tx: mpsc::Sender<UiEvent>) {
    std::thread::spawn(move || {
        loop {
            if crossterm::event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(ev) = crossterm::event::read() {
                    if ui_tx.blocking_send(UiEvent::Crossterm(ev)).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

fn spawn_tick(ui_tx: mpsc::Sender<UiEvent>, period: Duration) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(period);
        loop {
            interval.tick().await;
            if ui_tx.send(UiEvent::Tick).await.is_err() {
                break;
            }
        }
    });
}

fn handle_confirm_action(app: &mut AppState, action: ConfirmAction) {
    match action {
        ConfirmAction::StopExec { exec_id } => {
            let base_url = app.backend_url.clone();
            let net_tx = app.net_tx.clone();
            tokio::spawn(async move {
                match stop_exec_http(&base_url, exec_id).await {
                    Ok(()) => {}
                    Err(e) => {
                        let _ = net_tx
                            .send(NetEvent::Error(format!("stop exec failed: {e}")))
                            .await;
                    }
                }
            });
        }
    }
}

// Commands live in `commands`.
