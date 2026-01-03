use std::io;
use std::time::{Duration, Instant};

use anyhow::Context;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::text::Line;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use crate::commands::update_git_activity_indicators;
use crate::controller::handle_net_event;
use crate::diff_preview::{diff_preview_refresh_ready, request_diff_preview_async, schedule_diff_preview_refresh};
use crate::events::{NetEvent, UiEvent};
use crate::layout::{clamp_scroll_offsets, compute_main_layout, current_terminal_rect};
use crate::logs::{flush_log_buffers, mark_all_log_buffers_dirty};
use crate::prefs::load_prefs;
use crate::render;
use crate::state::{AppState, LogMode};
use crate::{Args, input, net, spawn_input_reader, spawn_tick};

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
                        let now = Instant::now();
                        let layout = compute_main_layout(current_terminal_rect());
                        let inner_width = layout.exec_logs.width.saturating_sub(2);
                        let width = inner_width as usize;
                        if app.exec.log_render_width != inner_width {
                            app.exec.log_render_width = inner_width;
                            mark_all_log_buffers_dirty(&mut app, 0);
                        }
                        if flush_log_buffers(&mut app, width) {
                            dirty = true;
                        }
                        // Throttle expensive diff preview rebuilds (highlighting/wrapping) during
                        // WS replay bursts by only refreshing when explicitly requested.
                        let diff_inner_width_u16 = layout.diff_preview.width.saturating_sub(2);
                        let diff_inner_width = diff_inner_width_u16 as usize;
                        let diff_width_changed =
                            app.diff.diff_preview_cache_width != diff_inner_width_u16;
                        let has_diffs = app
                            .diff
                            .diff_store
                            .get("entries")
                            .and_then(|v| v.as_object())
                            .is_some_and(|o| !o.is_empty());
                        if diff_width_changed {
                            app.diff.diff_preview_cache_width = diff_inner_width_u16;
                            app.diff.diff_preview_cache_key = None;
                            if has_diffs {
                                schedule_diff_preview_refresh(&mut app, Duration::from_millis(0));
                            }
                        }
                        if app.diff.diff_preview_cache_key.is_none()
                            && !app.diff.diff_preview_pending
                            && app.diff.diff_preview_job.is_none()
                            && has_diffs
                        {
                            schedule_diff_preview_refresh(&mut app, Duration::from_millis(0));
                        }
                        if diff_preview_refresh_ready(&app, now)
                            && app.diff.diff_preview_job.is_none()
                        {
                            if has_diffs {
                                request_diff_preview_async(&mut app, diff_inner_width);
                                dirty = true;
                            } else {
                                if app.diff.diff_preview_lines != vec![Line::from("No diffs")] {
                                    app.diff.diff_preview_lines = vec![Line::from("No diffs")];
                                    dirty = true;
                                }
                            }
                            app.diff.diff_preview_pending = false;
                            app.diff.diff_preview_next_refresh_at = None;
                        }
                        if clamp_scroll_offsets(&mut app, layout) {
                            dirty = true;
                        }
                        if update_git_activity_indicators(&mut app, now) {
                            dirty = true;
                        }
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
                        if input::handle_ui_event(&mut app, other)? {
                            break;
                        }
                        dirty = true;
                    }
                }
            }
            Some(ev) = net_rx.recv() => {
                handle_net_event(&mut app, ev);
                dirty = true;
            }
            else => break,
        }
    }

    Ok(())
}
