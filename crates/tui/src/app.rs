use std::io;
use std::time::{Duration, Instant};

use anyhow::Context;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use crate::events::{NetEvent, UiEvent};
use crate::layout::current_terminal_rect;
use crate::prefs::load_prefs;
use crate::render;
use crate::state::{AppState, LogMode};
use crate::{Args, net, spawn_input_reader, spawn_tick};
use crate::actions::{dispatch, Action};

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> anyhow::Result<Self> {
        enable_raw_mode().context("enable raw mode")?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)
            .context("enter alt screen")?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}

pub(crate) async fn run() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(if args.verbose { "debug" } else { "warn" })
        .init();

    let backend_url = net::resolve_backend_url(&args).await?;
    let _guard = TerminalGuard::enter()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).context("create terminal")?;
    terminal.clear().ok();

    let prefs = load_prefs();

    let (ui_tx, mut ui_rx) = mpsc::channel::<UiEvent>(256);
    let (net_tx, mut net_rx) = mpsc::channel::<NetEvent>(256);
    let (project_sel_tx, project_sel_rx) = watch::channel::<Option<Uuid>>(None);
    let (attempt_sel_tx, attempt_sel_rx) = watch::channel::<Option<Uuid>>(None);
    let (exec_sel_tx, exec_sel_rx) = watch::channel::<Option<Uuid>>(None);
    let (log_mode_tx, log_mode_rx) = watch::channel::<LogMode>(prefs.log_mode);
    let (diff_stats_tx, diff_stats_rx) = watch::channel::<bool>(false);
    let (diff_reconnect_tx, diff_reconnect_rx) = watch::channel::<u64>(0);
    let (reconnect_tx, reconnect_rx) = watch::channel::<u64>(0);

    spawn_input_reader(ui_tx.clone());
    // 30fps-ish tick; also used to batch WS patches + throttle redraw.
    spawn_tick(ui_tx.clone(), Duration::from_millis(33));

    tokio::spawn(net::load_info_task(backend_url.clone(), net_tx.clone()));
    tokio::spawn(net::projects_stream_task(
        backend_url.clone(),
        reconnect_rx.clone(),
        net_tx.clone(),
    ));
    tokio::spawn(net::tasks_stream_task(
        backend_url.clone(),
        project_sel_rx,
        reconnect_rx.clone(),
        net_tx.clone(),
    ));
    tokio::spawn(net::exec_stream_task(
        backend_url.clone(),
        attempt_sel_rx,
        reconnect_rx.clone(),
        net_tx.clone(),
    ));
    tokio::spawn(net::diff_stream_task(
        backend_url.clone(),
        attempt_sel_tx.subscribe(),
        diff_stats_rx,
        diff_reconnect_rx,
        reconnect_rx.clone(),
        net_tx.clone(),
    ));
    tokio::spawn(net::logs_stream_task(
        backend_url.clone(),
        exec_sel_rx,
        log_mode_rx,
        reconnect_rx.clone(),
        net_tx.clone(),
    ));

    let mut app = AppState::new(
        backend_url,
        net_tx.clone(),
        project_sel_tx,
        attempt_sel_tx,
        exec_sel_tx,
        log_mode_tx,
        diff_stats_tx,
        diff_reconnect_tx,
        reconnect_tx,
        prefs,
    );

    let mut dirty = true;
    loop {
        tokio::select! {
            Some(evt) = ui_rx.recv() => {
                match evt {
                    UiEvent::Tick => {
                        let out = dispatch(
                            &mut app,
                            Action::Tick {
                                now: Instant::now(),
                                term: current_terminal_rect(),
                            },
                        )?;
                        dirty |= out.dirty;
                        if dirty {
                            if app.ui.composer_active || app.ui.create_task.is_some() {
                                terminal.show_cursor().ok();
                            } else {
                                terminal.hide_cursor().ok();
                            }
                            terminal
                                .draw(|f| render::render(f, &app))
                                .context("draw frame")?;
                            dirty = false;
                        }
                    }
                    other => {
                        let out = dispatch(&mut app, Action::Ui(other))?;
                        if out.quit {
                            break;
                        }
                        dirty |= out.dirty;
                    }
                }
            }
            Some(ev) = net_rx.recv() => {
                let out = dispatch(&mut app, Action::Net(ev))?;
                dirty |= out.dirty;
            }
            else => break,
        }
    }

    Ok(())
}
