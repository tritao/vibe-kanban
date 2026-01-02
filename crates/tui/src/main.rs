use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io,
    path::PathBuf,
    sync::OnceLock,
    time::{Duration, Instant},
};

use anyhow::Context;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use pulldown_cmark::{
    CodeBlockKind, Event as MdEvent, Options as MdOptions, Parser as MdParser, Tag as MdTag,
    TagEnd as MdTagEnd,
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use serde::{Deserialize, Serialize};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite;
use unicode_width::UnicodeWidthStr;
use utils::{port_file::read_port_file, response::ApiResponse};
use uuid::Uuid;

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

#[derive(Debug)]
enum UiEvent {
    Crossterm(crossterm::event::Event),
    Tick,
}

#[derive(Debug)]
enum NetEvent {
    InfoLoaded {
        ok: bool,
        summary: String,
    },
    ProjectsStreamStatus(StreamStatus),
    ProjectsPatch(json_patch::Patch),
    TasksStreamStatus(StreamStatus),
    TasksReset,
    TasksPatch(json_patch::Patch),
    AttemptsLoaded {
        task_id: Uuid,
        attempts: Vec<AttemptRow>,
    },
    ExecStreamStatus(StreamStatus),
    ExecReset,
    ExecPatch(json_patch::Patch),
    DiffStreamStatus(StreamStatus),
    DiffReset,
    DiffPatch(json_patch::Patch),
    LogStreamStatus(StreamStatus),
    LogReset,
    LogPatch(json_patch::Patch),
    BranchStatusLoaded(Vec<RepoBranchStatus>),
    Notice(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamStatus {
    Connecting,
    Connected,
    Completed,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusPane {
    Board,
    Execution,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LogMode {
    Normalized,
    Raw,
}

impl LogMode {
    fn label(self) -> &'static str {
        match self {
            Self::Normalized => "normalized",
            Self::Raw => "raw",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LogRenderMode {
    Plain,
    Markdown,
}

impl LogRenderMode {
    fn label(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Markdown => "md",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffFocus {
    Files,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    SearchTasks,
}

#[derive(Debug, Clone)]
struct InputState {
    mode: InputMode,
    buffer: String,
    original: String,
}

#[derive(Debug, Clone, Copy)]
enum ConfirmAction {
    StopExec { exec_id: Uuid },
}

#[derive(Debug, Clone)]
struct ConfirmState {
    title: String,
    body: String,
    action: ConfirmAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct TuiPrefs {
    selected_project_id: Option<Uuid>,
    show_cancelled: bool,
    log_mode: LogMode,
    log_render_mode: LogRenderMode,
    diff_theme: DiffTheme,
    diff_wrap: bool,
}

impl Default for TuiPrefs {
    fn default() -> Self {
        Self {
            selected_project_id: None,
            show_cancelled: false,
            log_mode: LogMode::Normalized,
            log_render_mode: LogRenderMode::Markdown,
            diff_theme: DiffTheme::default(),
            diff_wrap: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DiffTheme {
    Base16OceanDark,
    Base16EightiesDark,
    Base16MochaDark,
    Base16OceanLight,
    InspiredGithub,
    SolarizedDark,
    SolarizedLight,
}

impl Default for DiffTheme {
    fn default() -> Self {
        Self::InspiredGithub
    }
}

impl DiffTheme {
    fn is_light(self) -> bool {
        matches!(
            self,
            Self::Base16OceanLight | Self::InspiredGithub | Self::SolarizedLight
        )
    }

    fn syntect_key(self) -> &'static str {
        match self {
            Self::Base16OceanDark => "base16-ocean.dark",
            Self::Base16EightiesDark => "base16-eighties.dark",
            Self::Base16MochaDark => "base16-mocha.dark",
            Self::Base16OceanLight => "base16-ocean.light",
            Self::InspiredGithub => "InspiredGitHub",
            Self::SolarizedDark => "Solarized (dark)",
            Self::SolarizedLight => "Solarized (light)",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Base16OceanDark => "ocean.dark",
            Self::Base16EightiesDark => "eighties.dark",
            Self::Base16MochaDark => "mocha.dark",
            Self::Base16OceanLight => "ocean.light",
            Self::InspiredGithub => "github",
            Self::SolarizedDark => "solarized.dark",
            Self::SolarizedLight => "solarized.light",
        }
    }

    fn cycle_next(self) -> Self {
        match self {
            Self::InspiredGithub => Self::Base16OceanDark,
            Self::Base16OceanDark => Self::Base16EightiesDark,
            Self::Base16EightiesDark => Self::Base16MochaDark,
            Self::Base16MochaDark => Self::Base16OceanLight,
            Self::Base16OceanLight => Self::SolarizedDark,
            Self::SolarizedDark => Self::SolarizedLight,
            Self::SolarizedLight => Self::InspiredGithub,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskStatus {
    Todo,
    InProgress,
    InReview,
    Done,
    Cancelled,
}

impl TaskStatus {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "todo" => Some(Self::Todo),
            "inprogress" => Some(Self::InProgress),
            "inreview" => Some(Self::InReview),
            "done" => Some(Self::Done),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    fn as_api_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "inprogress",
            Self::InReview => "inreview",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Todo => "Todo",
            Self::InProgress => "In Progress",
            Self::InReview => "In Review",
            Self::Done => "Done",
            Self::Cancelled => "Cancelled",
        }
    }

    fn idx(self) -> usize {
        match self {
            Self::Todo => 0,
            Self::InProgress => 1,
            Self::InReview => 2,
            Self::Done => 3,
            Self::Cancelled => 4,
        }
    }
}

#[derive(Debug, Clone)]
struct TaskRow {
    id: Uuid,
    title: String,
    status: TaskStatus,
    updated_at: Option<String>,
    has_in_progress_attempt: bool,
    last_attempt_failed: bool,
    executor: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone)]
struct AttemptRow {
    id: Uuid,
    branch: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    setup_completed_at: Option<String>,
}

#[derive(Debug, Clone)]
struct ExecRow {
    id: Uuid,
    session_id: Option<Uuid>,
    run_reason: Option<String>,
    status: Option<String>,
    created_at: Option<String>,
    dropped: bool,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConflictOp {
    Rebase,
    Merge,
    CherryPick,
    Revert,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum MergeStatus {
    Open,
    Merged,
    Closed,
    Unknown,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PullRequestInfo {
    number: i64,
    url: String,
    status: MergeStatus,
    #[allow(dead_code)]
    merged_at: Option<String>,
    #[allow(dead_code)]
    merge_commit_sha: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PrMerge {
    pr_info: PullRequestInfo,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Merge {
    #[allow(dead_code)]
    Direct(serde_json::Value),
    Pr(PrMerge),
}

#[derive(Debug, Clone, serde::Deserialize)]
struct BranchStatus {
    commits_behind: Option<usize>,
    commits_ahead: Option<usize>,
    has_uncommitted_changes: Option<bool>,
    uncommitted_count: Option<usize>,
    untracked_count: Option<usize>,
    target_branch_name: String,
    remote_commits_behind: Option<usize>,
    remote_commits_ahead: Option<usize>,
    merges: Vec<Merge>,
    is_rebase_in_progress: bool,
    conflict_op: Option<ConflictOp>,
    conflicted_files: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RepoBranchStatus {
    repo_id: Uuid,
    repo_name: String,
    #[serde(flatten)]
    status: BranchStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogKind {
    Stdout,
    Stderr,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgressKind {
    Thinking,
    Loading,
}

#[derive(Debug, Clone, Copy, Default)]
struct LogAssemblerState {
    open: bool,
    open_kind: Option<LogKind>,
    attach_to_entry: Option<usize>,
    progress_kind: Option<ProgressKind>,
    progress_count: usize,
    progress_line_pos: Option<usize>,
}

struct AppState {
    backend_url: String,
    info_summary: String,
    info_ok: bool,

    prefs: TuiPrefs,

    focus: FocusPane,
    show_help: bool,

    input: Option<InputState>,
    confirm: Option<ConfirmState>,

    project_filter: String,

    projects_status: StreamStatus,
    projects_store: serde_json::Value,
    selected_project_id: Option<Uuid>,
    selected_project_index: usize,

    task_filter: String,
    show_cancelled: bool,

    tasks_status: StreamStatus,
    tasks_store: serde_json::Value,
    selected_task_id: Option<Uuid>,
    tasks_active_column: TaskStatus,
    board_index_by_status: [usize; 5],

    attempts: Vec<AttemptRow>,
    selected_attempt_id: Option<Uuid>,
    selected_attempt_index: usize,

    exec_status: StreamStatus,
    exec_store: serde_json::Value,
    selected_exec_id: Option<Uuid>,

    diff_status: StreamStatus,
    diff_store: serde_json::Value,
    diff_stats_only: bool,
    selected_diff_index: usize,
    diff_scroll_offset: usize,
    diff_focus: DiffFocus,
    diff_preview_cache_key: Option<String>,
    diff_preview_cache_hash: u64,
    diff_preview_cache_width: u16,
    diff_preview_lines: Vec<Line<'static>>,
    diff_theme: DiffTheme,
    diff_wrap: bool,
    diff_preview_pending: bool,
    diff_preview_next_refresh_at: Option<Instant>,

    log_status: StreamStatus,
    log_store: serde_json::Value,
    log_lines: Vec<Line<'static>>,
    log_line_entry_index: Vec<usize>,
    log_entry_line_starts: Vec<usize>,
    log_entry_end_states: Vec<LogAssemblerState>,
    log_assembler_state: LogAssemblerState,
    pending_log_patch: json_patch::Patch,
    pending_log_dirty_from_entry: Option<usize>,
    log_collapsed: Vec<bool>,
    log_selected_entry: Option<usize>,
    log_mode: LogMode,
    log_render_mode: LogRenderMode,
    log_render_width: u16,
    log_autoscroll: bool,
    log_scroll_offset: usize,

    composer_active: bool,
    composer_buffer: String,
    composer_suggest_index: usize,

    last_error: Option<String>,
    last_notice: Option<String>,

    repo_statuses: Vec<RepoBranchStatus>,
    selected_repo_index: usize,

    net_tx: mpsc::Sender<NetEvent>,
    project_sel_tx: watch::Sender<Option<Uuid>>,
    attempt_sel_tx: watch::Sender<Option<Uuid>>,
    exec_sel_tx: watch::Sender<Option<Uuid>>,
    log_mode_tx: watch::Sender<LogMode>,
    diff_stats_tx: watch::Sender<bool>,
    reconnect_tx: watch::Sender<u64>,
}

impl AppState {
    fn new(
        backend_url: String,
        net_tx: mpsc::Sender<NetEvent>,
        project_sel_tx: watch::Sender<Option<Uuid>>,
        attempt_sel_tx: watch::Sender<Option<Uuid>>,
        exec_sel_tx: watch::Sender<Option<Uuid>>,
        log_mode_tx: watch::Sender<LogMode>,
        diff_stats_tx: watch::Sender<bool>,
        reconnect_tx: watch::Sender<u64>,
        prefs: TuiPrefs,
    ) -> Self {
        let state = Self {
            backend_url,
            info_summary: "loading /api/info…".to_string(),
            info_ok: false,

            prefs: prefs.clone(),

            focus: FocusPane::Board,
            show_help: false,

            input: None,
            confirm: None,

            project_filter: String::new(),

            projects_status: StreamStatus::Disconnected,
            projects_store: serde_json::json!({ "projects": {} }),
            selected_project_id: prefs.selected_project_id,
            selected_project_index: 0,

            task_filter: String::new(),
            show_cancelled: prefs.show_cancelled,

            tasks_status: StreamStatus::Disconnected,
            tasks_store: serde_json::json!({ "tasks": {} }),
            selected_task_id: None,
            tasks_active_column: TaskStatus::Todo,
            board_index_by_status: [0; 5],

            attempts: vec![],
            selected_attempt_id: None,
            selected_attempt_index: 0,

            exec_status: StreamStatus::Disconnected,
            exec_store: serde_json::json!({ "execution_processes": {} }),
            selected_exec_id: None,

            diff_status: StreamStatus::Disconnected,
            diff_store: serde_json::json!({ "entries": {} }),
            diff_stats_only: false,
            selected_diff_index: 0,
            diff_scroll_offset: 0,
            diff_focus: DiffFocus::Files,
            diff_preview_cache_key: None,
            diff_preview_cache_hash: 0,
            diff_preview_cache_width: 0,
            diff_preview_lines: vec![Line::from("No diffs")],
            diff_theme: prefs.diff_theme,
            diff_wrap: prefs.diff_wrap,
            diff_preview_pending: false,
            diff_preview_next_refresh_at: None,

            log_status: StreamStatus::Disconnected,
            log_store: serde_json::json!({ "entries": [] }),
            log_lines: vec![],
            log_line_entry_index: vec![],
            log_entry_line_starts: vec![],
            log_entry_end_states: vec![],
            log_assembler_state: LogAssemblerState::default(),
            pending_log_patch: json_patch::Patch::default(),
            pending_log_dirty_from_entry: None,
            log_collapsed: vec![],
            log_selected_entry: None,
            log_mode: prefs.log_mode,
            log_render_mode: prefs.log_render_mode,
            log_render_width: 0,
            log_autoscroll: true,
            log_scroll_offset: 0,

            composer_active: false,
            composer_buffer: String::new(),
            composer_suggest_index: 0,

            last_error: None,
            last_notice: None,

            repo_statuses: vec![],
            selected_repo_index: 0,

            net_tx,
            project_sel_tx,
            attempt_sel_tx,
            exec_sel_tx,
            log_mode_tx,
            diff_stats_tx,
            reconnect_tx,
        };

        let _ = state.project_sel_tx.send(state.selected_project_id);
        let _ = state.diff_stats_tx.send(state.diff_stats_only);

        state
    }
}

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(if args.verbose { "debug" } else { "warn" })
        .init();

    let backend_url = resolve_backend_url(&args).await?;
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
    let (reconnect_tx, reconnect_rx) = watch::channel::<u64>(0);

    spawn_input_reader(ui_tx.clone());
    // 30fps-ish tick; also used to batch WS patches + throttle redraw.
    spawn_tick(ui_tx.clone(), Duration::from_millis(33));

    tokio::spawn(load_info_task(backend_url.clone(), net_tx.clone()));
    tokio::spawn(projects_stream_task(
        backend_url.clone(),
        reconnect_rx.clone(),
        net_tx.clone(),
    ));
    tokio::spawn(tasks_stream_task(
        backend_url.clone(),
        project_sel_rx,
        reconnect_rx.clone(),
        net_tx.clone(),
    ));
    tokio::spawn(exec_stream_task(
        backend_url.clone(),
        attempt_sel_rx,
        reconnect_rx.clone(),
        net_tx.clone(),
    ));
    tokio::spawn(diff_stream_task(
        backend_url.clone(),
        attempt_sel_tx.subscribe(),
        diff_stats_rx,
        reconnect_rx.clone(),
        net_tx.clone(),
    ));
    tokio::spawn(logs_stream_task(
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
                        if app.log_render_width != inner_width {
                            app.log_render_width = inner_width;
                            app.pending_log_dirty_from_entry = Some(0);
                        }
                        if flush_log_patches(&mut app, width) {
                            dirty = true;
                        }
                        // Throttle expensive diff preview rebuilds (highlighting/wrapping) during
                        // WS replay bursts by only refreshing when explicitly requested.
                        let diff_inner_width_u16 = layout.diff_preview.width.saturating_sub(2);
                        let diff_inner_width = diff_inner_width_u16 as usize;
                        let diff_width_changed = app.diff_preview_cache_width != diff_inner_width_u16;
                        let should_refresh_diff =
                            diff_width_changed
                                || app.diff_preview_cache_key.is_none()
                                || diff_preview_refresh_ready(&app, now);
                        if should_refresh_diff {
                            if refresh_diff_preview_cache(&mut app, diff_inner_width) {
                                dirty = true;
                            }
                            app.diff_preview_pending = false;
                            app.diff_preview_next_refresh_at = None;
                        }
                        if clamp_scroll_offsets(&mut app, layout) {
                            dirty = true;
                        }
                        if dirty {
                            if app.composer_active {
                                terminal.show_cursor().ok();
                            } else {
                                terminal.hide_cursor().ok();
                            }
                            terminal.draw(|f| render(f, &app)).context("draw frame")?;
                            dirty = false;
                        }
                    }
                    other => {
                        if handle_ui_event(&mut app, other)? {
                            break;
                        }
                        dirty = true;
                    }
                }
            }
            Some(evt) = net_rx.recv() => {
                handle_net_event(&mut app, evt);
                dirty = true;
            }
            else => break,
        }
    }

    Ok(())
}

fn handle_net_event(app: &mut AppState, event: NetEvent) {
    match event {
        NetEvent::InfoLoaded { ok, summary } => {
            app.info_ok = ok;
            app.info_summary = summary;
        }
        NetEvent::ProjectsStreamStatus(status) => {
            app.projects_status = status;
        }
        NetEvent::ProjectsPatch(patch) => {
            if let Err(e) = json_patch::patch(&mut app.projects_store, &patch) {
                app.last_error = Some(format!("failed to apply projects patch: {e}"));
                app.projects_status = StreamStatus::Error;
                return;
            }

            let projects = filtered_projects(app);
            if projects.is_empty() {
                app.selected_project_index = 0;
                set_selected_project(app, None);
                return;
            }

            if let Some(selected_id) = app.selected_project_id {
                if let Some(idx) = projects.iter().position(|p| p.id == selected_id) {
                    app.selected_project_index = idx;
                    return;
                }
            }

            app.selected_project_index = app.selected_project_index.min(projects.len() - 1);
            set_selected_project(app, Some(projects[app.selected_project_index].id));
        }
        NetEvent::TasksStreamStatus(status) => {
            app.tasks_status = status;
        }
        NetEvent::TasksReset => {
            app.tasks_store = serde_json::json!({ "tasks": {} });
            set_selected_task(app, None);
        }
        NetEvent::TasksPatch(patch) => {
            if let Err(e) = json_patch::patch(&mut app.tasks_store, &patch) {
                app.last_error = Some(format!("failed to apply tasks patch: {e}"));
                app.tasks_status = StreamStatus::Error;
                return;
            }

            let tasks = tasks_filtered_base(app);
            if tasks.is_empty() {
                set_selected_task(app, None);
                return;
            }

            if let Some(selected_id) = app.selected_task_id {
                if tasks.iter().any(|t| t.id == selected_id) {
                    sync_tasks_active_column(app);
                    return;
                }
            }

            let by_status = tasks_by_status(&tasks);
            let chosen = match app.tasks_active_column {
                TaskStatus::Todo => by_status.todo.first(),
                TaskStatus::InProgress => by_status.inprogress.first(),
                TaskStatus::InReview => by_status.inreview.first(),
                TaskStatus::Done => by_status.done.first(),
                TaskStatus::Cancelled => by_status.cancelled.first(),
            }
            .or_else(|| tasks.first());

            set_selected_task(app, chosen.map(|t| t.id));
            sync_tasks_active_column(app);
            ensure_selection_visible(app);
        }
        NetEvent::AttemptsLoaded { task_id, attempts } => {
            if app.selected_task_id != Some(task_id) {
                return;
            }

            app.attempts = attempts;
            app.selected_attempt_index = 0;
            let default_attempt = app.attempts.first().map(|a| a.id);
            set_selected_attempt(app, default_attempt);
        }
        NetEvent::ExecStreamStatus(status) => {
            app.exec_status = status;
        }
        NetEvent::ExecReset => {
            app.exec_store = serde_json::json!({ "execution_processes": {} });
            set_selected_exec(app, None);
        }
        NetEvent::ExecPatch(patch) => {
            if let Err(e) = json_patch::patch(&mut app.exec_store, &patch) {
                app.last_error = Some(format!("failed to apply exec patch: {e}"));
                app.exec_status = StreamStatus::Error;
                return;
            }

            let execs = exec_list(&app.exec_store);
            set_selected_exec(app, active_exec_id(&execs));
        }
        NetEvent::DiffStreamStatus(status) => {
            app.diff_status = status;
            if status == StreamStatus::Completed && app.diff_preview_pending {
                // If we were throttling refreshes during replay, do one final refresh ASAP.
                app.diff_preview_next_refresh_at = Some(Instant::now());
            }
        }
        NetEvent::DiffReset => {
            app.diff_store = serde_json::json!({ "entries": {} });
            app.selected_diff_index = 0;
            app.diff_scroll_offset = 0;
            app.diff_preview_cache_key = None;
            app.diff_preview_pending = false;
            app.diff_preview_next_refresh_at = None;
        }
        NetEvent::DiffPatch(patch) => {
            let prev_selected_key = {
                let prev_rows = diff_rows_with_all(&app.diff_store);
                prev_rows
                    .get(
                        app.selected_diff_index
                            .min(prev_rows.len().saturating_sub(1)),
                    )
                    .map(|r| r.key.clone())
            };

            if let Err(e) = json_patch::patch(&mut app.diff_store, &patch) {
                app.last_error = Some(format!("failed to apply diff patch: {e}"));
                app.diff_status = StreamStatus::Error;
                return;
            }

            let rows = diff_rows_with_all(&app.diff_store);
            if rows.is_empty() {
                app.selected_diff_index = 0;
            } else {
                app.selected_diff_index = app.selected_diff_index.min(rows.len() - 1);
            }

            // Only refresh diff preview if the patch affects the currently selected entry.
            // This avoids re-highlighting on every patch during server rebuild/replay bursts.
            let selected = rows.get(app.selected_diff_index);
            let selection_changed =
                prev_selected_key.as_deref() != selected.map(|s| s.key.as_str());
            if let Some(selected) = selected {
                let needs_refresh = selection_changed
                    || if selected.key == DIFF_ALL_KEY {
                        true
                    } else {
                        diff_patch_touches_key(&patch, &selected.key)
                    };

                if needs_refresh {
                    let delay = match app.diff_status {
                        StreamStatus::Completed => Duration::from_millis(0),
                        _ => Duration::from_millis(75),
                    };
                    schedule_diff_preview_refresh(app, delay);
                }
            } else if selection_changed {
                // Diffs disappeared; refresh the preview to show the empty state.
                schedule_diff_preview_refresh(app, Duration::from_millis(0));
            }
        }
        NetEvent::LogStreamStatus(status) => {
            app.log_status = status;
        }
        NetEvent::LogReset => {
            reset_logs(app);
        }
        NetEvent::LogPatch(patch) => {
            enqueue_log_patch(app, patch);
        }
        NetEvent::BranchStatusLoaded(statuses) => {
            app.repo_statuses = statuses;
            if app.repo_statuses.is_empty() {
                app.selected_repo_index = 0;
            } else {
                app.selected_repo_index = app.selected_repo_index.min(app.repo_statuses.len() - 1);
            }
        }
        NetEvent::Notice(msg) => {
            app.last_notice = Some(msg);
        }
        NetEvent::Error(msg) => {
            app.last_error = Some(msg);
        }
    }
}

fn handle_ui_event(app: &mut AppState, event: UiEvent) -> anyhow::Result<bool> {
    match event {
        UiEvent::Tick => Ok(false),
        UiEvent::Crossterm(ev) => match ev {
            crossterm::event::Event::Key(key) => {
                use crossterm::event::{KeyCode, KeyModifiers};

                if let Some(confirm) = app.confirm.as_ref() {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Enter => {
                            let action = confirm.action;
                            app.confirm = None;
                            handle_confirm_action(app, action);
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            app.confirm = None;
                        }
                        _ => {}
                    }
                    return Ok(false);
                }

                if let Some(input) = app.input.as_mut() {
                    match (key.code, key.modifiers) {
                        (KeyCode::Esc, _) => {
                            app.task_filter = input.original.clone();
                            app.input = None;
                            ensure_selection_visible(app);
                        }
                        (KeyCode::Enter, _) => {
                            app.input = None;
                            ensure_selection_visible(app);
                        }
                        (KeyCode::Backspace, _) => {
                            input.buffer.pop();
                            app.task_filter = input.buffer.clone();
                            ensure_selection_visible(app);
                        }
                        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                            input.buffer.clear();
                            app.task_filter.clear();
                            ensure_selection_visible(app);
                        }
                        (KeyCode::Char(c), KeyModifiers::NONE) => {
                            input.buffer.push(c);
                            app.task_filter = input.buffer.clone();
                            ensure_selection_visible(app);
                        }
                        _ => {}
                    }
                    return Ok(false);
                }

                if app.show_help {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc => {
                            app.show_help = false;
                        }
                        _ => {}
                    }
                    return Ok(false);
                }

                if app.composer_active {
                    match (key.code, key.modifiers) {
                        (KeyCode::Esc, _) => {
                            app.composer_active = false;
                            app.composer_buffer.clear();
                            app.composer_suggest_index = 0;
                        }
                        (KeyCode::Enter, _) => {
                            submit_composer(app);
                        }
                        (KeyCode::Tab, _) => {
                            if apply_composer_autocomplete(app) {
                                // keep composing
                            }
                        }
                        (KeyCode::Up, _) => {
                            move_composer_autocomplete(app, -1);
                        }
                        (KeyCode::Down, _) => {
                            move_composer_autocomplete(app, 1);
                        }
                        (KeyCode::Backspace, _) => {
                            app.composer_buffer.pop();
                            app.composer_suggest_index = 0;
                        }
                        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                            app.composer_buffer.clear();
                            app.composer_suggest_index = 0;
                        }
                        (KeyCode::Char(c), KeyModifiers::NONE) => {
                            app.composer_buffer.push(c);
                            app.composer_suggest_index = 0;
                        }
                        _ => {}
                    }
                    return Ok(false);
                }

                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) => return Ok(true),
                    (KeyCode::Char('?'), _) => {
                        app.show_help = true;
                    }
                    (KeyCode::Tab, KeyModifiers::NONE) => {
                        app.focus = match app.focus {
                            FocusPane::Board => FocusPane::Execution,
                            FocusPane::Execution => FocusPane::Diff,
                            FocusPane::Diff => FocusPane::Board,
                        };
                    }
                    (KeyCode::Char('/'), _) => {
                        app.input = Some(InputState {
                            mode: InputMode::SearchTasks,
                            buffer: app.task_filter.clone(),
                            original: app.task_filter.clone(),
                        });
                    }
                    (KeyCode::Char('r'), _) => {
                        let next = *app.reconnect_tx.borrow() + 1;
                        let _ = app.reconnect_tx.send(next);
                    }
                    (KeyCode::Char('o'), _) => {
                        app.log_mode = match app.log_mode {
                            LogMode::Normalized => LogMode::Raw,
                            LogMode::Raw => LogMode::Normalized,
                        };
                        let _ = app.log_mode_tx.send(app.log_mode);
                        app.prefs.log_mode = app.log_mode;
                        save_prefs(&app.prefs);
                    }
                    (KeyCode::Char('m'), _) if app.focus == FocusPane::Execution => {
                        app.log_render_mode = match app.log_render_mode {
                            LogRenderMode::Plain => LogRenderMode::Markdown,
                            LogRenderMode::Markdown => LogRenderMode::Plain,
                        };
                        app.prefs.log_render_mode = app.log_render_mode;
                        save_prefs(&app.prefs);
                        app.pending_log_dirty_from_entry = Some(0);
                    }
                    (KeyCode::Char('e'), _) | (KeyCode::Enter, _)
                        if app.focus == FocusPane::Execution =>
                    {
                        toggle_selected_log_entry(app);
                    }
                    (KeyCode::Char('d'), _) => {
                        app.diff_stats_only = !app.diff_stats_only;
                        let _ = app.diff_stats_tx.send(app.diff_stats_only);
                        app.diff_scroll_offset = 0;
                    }
                    (KeyCode::Char('t'), _) if app.focus == FocusPane::Diff => {
                        app.diff_theme = app.diff_theme.cycle_next();
                        app.prefs.diff_theme = app.diff_theme;
                        save_prefs(&app.prefs);
                        app.diff_preview_cache_key = None;
                    }
                    (KeyCode::Char('i'), _) if app.focus == FocusPane::Execution => {
                        app.composer_active = true;
                        app.composer_suggest_index = 0;
                    }
                    (KeyCode::Char('x'), _) => {
                        if let Some(exec_id) = app.selected_exec_id {
                            app.confirm = Some(ConfirmState {
                                title: "Stop execution?".to_string(),
                                body: format!("Stop execution process {exec_id}? (y/n)"),
                                action: ConfirmAction::StopExec { exec_id },
                            });
                        }
                    }
                    (KeyCode::Char('['), _) => {
                        select_adjacent_attempt(app, -1);
                    }
                    (KeyCode::Char(']'), _) => {
                        select_adjacent_attempt(app, 1);
                    }
                    (KeyCode::Char('h'), _) if app.focus == FocusPane::Diff => {
                        app.diff_focus = DiffFocus::Files;
                    }
                    (KeyCode::Char('l'), _) if app.focus == FocusPane::Diff => {
                        app.diff_focus = DiffFocus::Preview;
                    }
                    (KeyCode::Char('w'), _) if app.focus == FocusPane::Diff => {
                        app.diff_wrap = !app.diff_wrap;
                        app.prefs.diff_wrap = app.diff_wrap;
                        save_prefs(&app.prefs);
                        app.diff_preview_cache_key = None;
                    }
                    (KeyCode::Char('S'), _) if app.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::RefreshStatus);
                    }
                    (KeyCode::Char('M'), _) if app.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::Merge);
                    }
                    (KeyCode::Char('R'), _) if app.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::Rebase);
                    }
                    (KeyCode::Char('P'), _) if app.focus == FocusPane::Diff => {
                        trigger_diff_repo_action(app, DiffRepoAction::CreatePr);
                    }
                    (KeyCode::Char('c'), _) if app.focus == FocusPane::Board => {
                        app.show_cancelled = !app.show_cancelled;
                        if !app.show_cancelled && app.tasks_active_column == TaskStatus::Cancelled {
                            app.tasks_active_column = TaskStatus::Done;
                        }
                        app.prefs.show_cancelled = app.show_cancelled;
                        save_prefs(&app.prefs);
                    }
                    (KeyCode::Char('K'), _) if app.focus == FocusPane::Board => {
                        move_active_status(app, -1);
                    }
                    (KeyCode::Char('J'), _) if app.focus == FocusPane::Board => {
                        move_active_status(app, 1);
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _) if app.focus == FocusPane::Board => {
                        select_adjacent_task(app, -1);
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _)
                        if app.focus == FocusPane::Board =>
                    {
                        select_adjacent_task(app, 1);
                    }
                    (KeyCode::Left, _) if app.focus == FocusPane::Board => {
                        request_move_selected_task(app, -1);
                    }
                    (KeyCode::Right, _) if app.focus == FocusPane::Board => {
                        request_move_selected_task(app, 1);
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _)
                        if app.focus == FocusPane::Diff && app.diff_focus == DiffFocus::Files =>
                    {
                        select_adjacent_diff_file(app, -1);
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _)
                        if app.focus == FocusPane::Diff && app.diff_focus == DiffFocus::Files =>
                    {
                        select_adjacent_diff_file(app, 1);
                    }
                    (KeyCode::PageUp, _) if app.focus == FocusPane::Execution => {
                        app.log_autoscroll = false;
                        app.log_scroll_offset = app.log_scroll_offset.saturating_add(40);
                    }
                    (KeyCode::PageDown, _) if app.focus == FocusPane::Execution => {
                        app.log_scroll_offset = app.log_scroll_offset.saturating_sub(40);
                        if app.log_scroll_offset == 0 {
                            app.log_autoscroll = true;
                        }
                    }
                    (KeyCode::End, _) if app.focus == FocusPane::Execution => {
                        app.log_autoscroll = true;
                        app.log_scroll_offset = 0;
                    }
                    (KeyCode::PageUp, _) if app.focus == FocusPane::Diff => {
                        app.diff_scroll_offset = app.diff_scroll_offset.saturating_sub(20);
                    }
                    (KeyCode::PageDown, _) if app.focus == FocusPane::Diff => {
                        app.diff_scroll_offset = app.diff_scroll_offset.saturating_add(20);
                    }
                    _ => {}
                }
                Ok(false)
            }
            crossterm::event::Event::Mouse(mouse) => {
                handle_mouse_event(app, mouse);
                Ok(false)
            }
            _ => Ok(false),
        },
    }
}

fn current_terminal_rect() -> ratatui::layout::Rect {
    let (w, h) = crossterm::terminal::size().unwrap_or((80, 24));
    ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    }
}

fn rect_contains(r: ratatui::layout::Rect, col: u16, row: u16) -> bool {
    col >= r.x
        && col < r.x.saturating_add(r.width)
        && row >= r.y
        && row < r.y.saturating_add(r.height)
}

#[derive(Debug, Clone, Copy)]
struct MainLayoutRects {
    board: ratatui::layout::Rect,
    exec: ratatui::layout::Rect,
    diff: ratatui::layout::Rect,
    exec_logs: ratatui::layout::Rect,
    exec_input: ratatui::layout::Rect,
    #[allow(dead_code)]
    diff_repo_bar: ratatui::layout::Rect,
    diff_files: ratatui::layout::Rect,
    diff_preview: ratatui::layout::Rect,
}

fn compute_main_layout(area: ratatui::layout::Rect) -> MainLayoutRects {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(18),
            Constraint::Percentage(54),
            Constraint::Percentage(28),
        ])
        .split(root[1]);

    let exec_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(main[1]);

    let diff_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Min(3),
        ])
        .split(main[2]);

    MainLayoutRects {
        board: main[0],
        exec: main[1],
        diff: main[2],
        exec_logs: exec_sections[0],
        exec_input: exec_sections[1],
        diff_repo_bar: diff_sections[0],
        diff_files: diff_sections[1],
        diff_preview: diff_sections[2],
    }
}

fn json_pointer_escape_segment(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

fn diff_patch_touches_key(patch: &json_patch::Patch, key: &str) -> bool {
    let escaped = json_pointer_escape_segment(key);
    let prefix = format!("/entries/{escaped}");
    patch.iter().any(|op| {
        let path = op.path().to_string();
        path == "/entries" || path == prefix || path.starts_with(&(prefix.clone() + "/"))
    })
}

fn schedule_diff_preview_refresh(app: &mut AppState, delay: Duration) {
    let now = Instant::now();
    let next = now + delay;
    app.diff_preview_pending = true;
    app.diff_preview_next_refresh_at = match app.diff_preview_next_refresh_at {
        Some(existing) => Some(existing.min(next)),
        None => Some(next),
    };
}

fn diff_preview_refresh_ready(app: &AppState, now: Instant) -> bool {
    app.diff_preview_pending
        && app
            .diff_preview_next_refresh_at
            .map(|t| now >= t)
            .unwrap_or(true)
}

fn clamp_scroll_offsets(app: &mut AppState, layout: MainLayoutRects) -> bool {
    // Prevent internal offsets from growing beyond the maximum (overscroll), which would require
    // scrolling back down the same amount before the viewport starts moving again.
    let mut changed = false;

    // Execution log pane (offset counts "lines above the viewport", i.e. distance from bottom).
    {
        let len = app.log_lines.len();
        let height = layout.exec_logs.height.saturating_sub(2) as usize;
        let visible = height.min(len);
        let max_offset = len.saturating_sub(visible);

        if app.log_autoscroll {
            if app.log_scroll_offset != 0 {
                app.log_scroll_offset = 0;
                changed = true;
            }
        } else {
            let next = app.log_scroll_offset.min(max_offset);
            if next != app.log_scroll_offset {
                app.log_scroll_offset = next;
                changed = true;
            }
            if app.log_scroll_offset == 0 && !app.log_autoscroll {
                app.log_autoscroll = true;
                changed = true;
            }
        }
    }

    // Diff preview pane (offset counts "first visible line").
    {
        let len = app.diff_preview_lines.len();
        let height = layout.diff_preview.height.saturating_sub(2) as usize;
        let visible = height.min(len);
        let max_start = len.saturating_sub(visible);

        let next = app.diff_scroll_offset.min(max_start);
        if next != app.diff_scroll_offset {
            app.diff_scroll_offset = next;
            changed = true;
        }
    }

    changed
}

#[derive(Debug, Clone, Copy)]
struct BoardHit {
    status: TaskStatus,
    clicked_index: Option<usize>,
    clicked_task_id: Option<Uuid>,
}

fn board_hit_at(
    app: &AppState,
    area: ratatui::layout::Rect,
    col: u16,
    row: u16,
) -> Option<BoardHit> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let tasks = tasks_filtered_base(app);
    let by_status = tasks_by_status(&tasks);
    let statuses = board_statuses(app);
    let needs: Vec<u16> = statuses
        .iter()
        .map(|status| {
            let list_len = match status {
                TaskStatus::Todo => by_status.todo.len(),
                TaskStatus::InProgress => by_status.inprogress.len(),
                TaskStatus::InReview => by_status.inreview.len(),
                TaskStatus::Done => by_status.done.len(),
                TaskStatus::Cancelled => by_status.cancelled.len(),
            };
            desired_board_section_height(list_len)
        })
        .collect();
    let heights = allocate_board_section_heights(&needs, area.height);
    let mut constraints: Vec<Constraint> = heights
        .iter()
        .copied()
        .map(|h| Constraint::Length(h))
        .collect();
    constraints.push(Constraint::Min(0));

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (idx, status) in statuses.iter().copied().enumerate() {
        if idx >= sections.len() {
            break;
        }
        let rect = sections[idx];
        if !rect_contains(rect, col, row) {
            continue;
        }

        let list: &[TaskRow] = match status {
            TaskStatus::Todo => &by_status.todo,
            TaskStatus::InProgress => &by_status.inprogress,
            TaskStatus::InReview => &by_status.inreview,
            TaskStatus::Done => &by_status.done,
            TaskStatus::Cancelled => &by_status.cancelled,
        };

        if list.is_empty() {
            return Some(BoardHit {
                status,
                clicked_index: None,
                clicked_task_id: None,
            });
        }

        let inner_y0 = rect.y.saturating_add(1);
        let inner_y1 = rect.y.saturating_add(rect.height).saturating_sub(1);
        if row < inner_y0 || row >= inner_y1 {
            return Some(BoardHit {
                status,
                clicked_index: None,
                clicked_task_id: None,
            });
        }

        let height = rect.height.saturating_sub(2) as usize;
        if height == 0 {
            return Some(BoardHit {
                status,
                clicked_index: None,
                clicked_task_id: None,
            });
        }

        let is_active = status == app.tasks_active_column;
        let selected_idx = if is_active {
            task_index_in(list, app.selected_task_id)
                .or_else(|| {
                    (!list.is_empty()).then_some(
                        app.board_index_by_status[status.idx()].min(list.len().saturating_sub(1)),
                    )
                })
                .unwrap_or(0)
        } else {
            0
        };

        let (start, end, _) = window_for_list(list.len(), selected_idx, height);
        let visible_len = end.saturating_sub(start);
        let inner_row = row.saturating_sub(inner_y0) as usize;
        if inner_row >= visible_len {
            return Some(BoardHit {
                status,
                clicked_index: None,
                clicked_task_id: None,
            });
        }

        let clicked_index = start + inner_row;
        let clicked_task_id = list.get(clicked_index).map(|t| t.id);
        return Some(BoardHit {
            status,
            clicked_index: Some(clicked_index),
            clicked_task_id,
        });
    }

    None
}

fn diff_files_hit_at(
    app: &AppState,
    area: ratatui::layout::Rect,
    col: u16,
    row: u16,
) -> Option<usize> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let rows = diff_rows_with_all(&app.diff_store);
    if rows.is_empty() {
        return None;
    }

    let inner_y0 = area.y.saturating_add(1);
    let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
    if row < inner_y0 || row >= inner_y1 {
        return None;
    }

    let height = area.height.saturating_sub(2) as usize;
    if height == 0 {
        return None;
    }

    let selected = app.selected_diff_index.min(rows.len() - 1);
    let (start, end, _) = window_for_list(rows.len(), selected, height);
    let visible_len = end.saturating_sub(start);
    let inner_row = row.saturating_sub(inner_y0) as usize;
    if inner_row >= visible_len {
        return None;
    }

    Some(start + inner_row)
}

fn log_entry_hit_at(
    app: &AppState,
    area: ratatui::layout::Rect,
    col: u16,
    row: u16,
) -> Option<usize> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let len = app.log_lines.len();
    if len == 0 {
        return None;
    }

    let inner_y0 = area.y.saturating_add(1);
    let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
    if row < inner_y0 || row >= inner_y1 {
        return None;
    }

    let visible = area.height.saturating_sub(2) as usize;
    if visible == 0 {
        return None;
    }

    let visible = visible.min(len);
    let mut offset = if app.log_autoscroll {
        0
    } else {
        app.log_scroll_offset
    };
    offset = offset.min(len.saturating_sub(visible));
    let start = len.saturating_sub(visible + offset);

    let inner_row = row.saturating_sub(inner_y0) as usize;
    if inner_row >= visible {
        return None;
    }

    let line_idx = start.saturating_add(inner_row);
    app.log_line_entry_index.get(line_idx).copied()
}

fn handle_mouse_event(app: &mut AppState, mouse: crossterm::event::MouseEvent) {
    if app.confirm.is_some() || app.input.is_some() || app.show_help {
        return;
    }

    use crossterm::event::{MouseButton, MouseEventKind};

    let col = mouse.column;
    let row = mouse.row;
    let layout = compute_main_layout(current_terminal_rect());

    const LOG_WHEEL_STEP: usize = 3;
    const DIFF_WHEEL_STEP: usize = 3;

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if rect_contains(layout.exec_logs, col, row) {
                app.focus = FocusPane::Execution;
                app.log_autoscroll = false;
                app.log_scroll_offset = app.log_scroll_offset.saturating_add(LOG_WHEEL_STEP);
                return;
            }
            if rect_contains(layout.diff_preview, col, row) {
                app.focus = FocusPane::Diff;
                app.diff_focus = DiffFocus::Preview;
                app.diff_scroll_offset = app.diff_scroll_offset.saturating_sub(DIFF_WHEEL_STEP);
                return;
            }
            if rect_contains(layout.diff_files, col, row) {
                app.focus = FocusPane::Diff;
                app.diff_focus = DiffFocus::Files;
                select_adjacent_diff_file(app, -1);
                return;
            }
            if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                app.focus = FocusPane::Board;
                app.tasks_active_column = hit.status;
                ensure_selected_task_in_active_column(app);
                select_adjacent_task(app, -1);
            }
        }
        MouseEventKind::ScrollDown => {
            if rect_contains(layout.exec_logs, col, row) {
                app.focus = FocusPane::Execution;
                app.log_scroll_offset = app.log_scroll_offset.saturating_sub(LOG_WHEEL_STEP);
                if app.log_scroll_offset == 0 {
                    app.log_autoscroll = true;
                }
                return;
            }
            if rect_contains(layout.diff_preview, col, row) {
                app.focus = FocusPane::Diff;
                app.diff_focus = DiffFocus::Preview;
                app.diff_scroll_offset = app.diff_scroll_offset.saturating_add(DIFF_WHEEL_STEP);
                return;
            }
            if rect_contains(layout.diff_files, col, row) {
                app.focus = FocusPane::Diff;
                app.diff_focus = DiffFocus::Files;
                select_adjacent_diff_file(app, 1);
                return;
            }
            if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                app.focus = FocusPane::Board;
                app.tasks_active_column = hit.status;
                ensure_selected_task_in_active_column(app);
                select_adjacent_task(app, 1);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if rect_contains(layout.board, col, row) {
                app.focus = FocusPane::Board;
                if let Some(hit) = board_hit_at(app, layout.board, col, row) {
                    app.tasks_active_column = hit.status;
                    if let Some(idx) = hit.clicked_index {
                        app.board_index_by_status[hit.status.idx()] = idx;
                    }
                    if let Some(task_id) = hit.clicked_task_id {
                        set_selected_task(app, Some(task_id));
                    } else {
                        ensure_selected_task_in_active_column(app);
                    }
                }
                return;
            }

            if rect_contains(layout.exec, col, row) {
                app.focus = FocusPane::Execution;
                if rect_contains(layout.exec_input, col, row) {
                    app.composer_active = true;
                } else if rect_contains(layout.exec_logs, col, row) {
                    app.log_selected_entry = log_entry_hit_at(app, layout.exec_logs, col, row);
                }
                return;
            }

            if rect_contains(layout.diff, col, row) {
                app.focus = FocusPane::Diff;
                if rect_contains(layout.diff_repo_bar, col, row) {
                    if let Some(action) =
                        diff_repo_bar_action_at(app, layout.diff_repo_bar, col, row)
                    {
                        trigger_diff_repo_action(app, action);
                    }
                    return;
                }
                if rect_contains(layout.diff_files, col, row) {
                    app.diff_focus = DiffFocus::Files;
                    if let Some(idx) = diff_files_hit_at(app, layout.diff_files, col, row) {
                        if idx != app.selected_diff_index {
                            app.selected_diff_index = idx;
                            app.diff_scroll_offset = 0;
                            sync_selected_repo_from_diff_selection(app);
                            schedule_diff_preview_refresh(app, Duration::from_millis(0));
                        }
                    }
                } else if rect_contains(layout.diff_preview, col, row) {
                    app.diff_focus = DiffFocus::Preview;
                }
                return;
            }
        }
        MouseEventKind::Down(MouseButton::Right) => {
            if rect_contains(layout.exec_logs, col, row) {
                app.focus = FocusPane::Execution;
                app.log_selected_entry = log_entry_hit_at(app, layout.exec_logs, col, row);
                toggle_selected_log_entry(app);
                return;
            }
        }
        _ => {}
    }
}

fn render(f: &mut Frame, app: &AppState) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());

    let top = render_top_bar(app);
    f.render_widget(top, root[0]);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(18),
            Constraint::Percentage(54),
            Constraint::Percentage(28),
        ])
        .split(root[1]);

    render_board_pane(f, app, main[0]);
    render_execution_pane(f, app, main[1]);
    render_diff_pane(f, app, main[2]);
    render_composer_autocomplete(f, app, compute_main_layout(f.area()).exec_input);

    let bottom = render_bottom_bar(app);
    f.render_widget(bottom, root[2]);

    if app.show_help {
        render_help_modal(f);
    }

    if let Some(confirm) = app.confirm.as_ref() {
        render_confirm_modal(f, confirm);
    }

    if let Some(input) = app.input.as_ref() {
        render_input_modal(f, input);
    }
}

fn render_top_bar(app: &AppState) -> Paragraph<'static> {
    fn status_badge(label: &'static str, status: StreamStatus) -> Span<'static> {
        let text = match status {
            StreamStatus::Connecting => format!("{label}:…"),
            StreamStatus::Connected => format!("{label}:ok"),
            StreamStatus::Completed => format!("{label}:done"),
            StreamStatus::Disconnected => format!("{label}:off"),
            StreamStatus::Error => format!("{label}:err"),
        };
        let color = match status {
            StreamStatus::Connecting => Color::Yellow,
            StreamStatus::Connected => Color::Green,
            StreamStatus::Completed => Color::Green,
            StreamStatus::Disconnected => Color::DarkGray,
            StreamStatus::Error => Color::Red,
        };
        Span::styled(text, Style::default().fg(color))
    }

    let focus = match app.focus {
        FocusPane::Board => "board",
        FocusPane::Execution => "exec",
        FocusPane::Diff => "diff",
    };

    let project_name = app
        .selected_project_id
        .and_then(|id| {
            app.projects_store
                .get("projects")?
                .get(id.to_string())?
                .get("name")?
                .as_str()
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "(no project)".to_string());

    let task_title = app
        .selected_task_id
        .and_then(|id| find_task(&app.tasks_store, id).map(|t| t.title))
        .unwrap_or_else(|| "—".to_string());

    let attempt_branch = app
        .selected_attempt_id
        .and_then(|id| app.attempts.iter().find(|a| a.id == id))
        .map(|a| a.branch.clone())
        .unwrap_or_else(|| "—".to_string());

    let line = Line::from(vec![
        Span::styled("vk-tui", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(
            truncate(&project_name, 18),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  "),
        Span::styled(truncate(&task_title, 28), Style::default()),
        Span::raw("  "),
        Span::styled(
            truncate(&attempt_branch, 18),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw("  "),
        status_badge("tasks", app.tasks_status),
        Span::raw(" "),
        status_badge("exec", app.exec_status),
        Span::raw(" "),
        status_badge("diff", app.diff_status),
        Span::raw(" "),
        status_badge("log", app.log_status),
        Span::raw("  "),
        Span::styled(
            format!("mode:{}", app.log_mode.label()),
            Style::default().fg(Color::Gray),
        ),
        Span::raw(" "),
        Span::styled(
            format!("view:{}", app.log_render_mode.label()),
            Style::default().fg(Color::Gray),
        ),
        Span::raw("  "),
        Span::styled(focus, Style::default().add_modifier(Modifier::DIM)),
    ]);

    Paragraph::new(line)
}

fn render_bottom_bar(app: &AppState) -> Paragraph<'static> {
    let text = match app.focus {
        FocusPane::Board => {
            "Tab next | j/k move | J/K status | ←/→ move | / search | [/] attempts | x stop | o log mode | q quit"
        }
        FocusPane::Execution => {
            "Tab next | i compose (/cmd) | Enter send | e expand | PgUp/PgDn scroll | End bottom | m md view | x stop | o log mode | q quit"
        }
        FocusPane::Diff => {
            "Tab next | j/k file | h/l files/preview | PgUp/PgDn scroll | d stats-only | t theme | w wrap | M merge | P PR | R rebase | S status | q quit"
        }
    };
    Paragraph::new(Line::from(Span::styled(
        text,
        Style::default().add_modifier(Modifier::DIM),
    )))
}

fn board_statuses(app: &AppState) -> Vec<TaskStatus> {
    if app.show_cancelled {
        vec![
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::InReview,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ]
    } else {
        vec![
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::InReview,
            TaskStatus::Done,
        ]
    }
}

fn window_for_list(len: usize, selected: usize, height: usize) -> (usize, usize, usize) {
    if len == 0 || height == 0 {
        return (0, 0, 0);
    }

    if len <= height {
        return (0, len, selected.min(len - 1));
    }

    let selected = selected.min(len - 1);
    let mut start = selected.saturating_sub(height / 2);
    start = start.min(len.saturating_sub(height));
    let end = (start + height).min(len);
    (start, end, selected.saturating_sub(start))
}

fn desired_board_section_height(list_len: usize) -> u16 {
    let inner = (list_len.max(1)).min(u16::MAX as usize) as u16;
    inner.saturating_add(2).max(3)
}

fn allocate_board_section_heights(needs: &[u16], available: u16) -> Vec<u16> {
    if needs.is_empty() || available == 0 {
        return vec![];
    }

    // 3 lines is the minimum to show a bordered block + 1 line of content.
    let min_h = 3u16;
    let n = needs.len();

    // If the terminal is absurdly small, just split whatever is available.
    if available < (n as u16).saturating_mul(min_h) {
        let base = (available / n as u16).max(1);
        let mut heights = vec![base; n];
        let mut remaining = available.saturating_sub(base.saturating_mul(n as u16));
        for h in heights.iter_mut() {
            if remaining == 0 {
                break;
            }
            *h = h.saturating_add(1);
            remaining -= 1;
        }
        return heights;
    }

    let needs: Vec<u16> = needs.iter().copied().map(|h| h.max(min_h)).collect();
    let total_need: u16 = needs.iter().copied().sum();
    if total_need <= available {
        return needs;
    }

    let mut heights = vec![min_h; n];
    let mut remaining = available.saturating_sub(min_h.saturating_mul(n as u16));
    let mut deficits: Vec<u16> = needs.iter().map(|h| h.saturating_sub(min_h)).collect();

    while remaining > 0 {
        let mut progressed = false;
        for i in 0..n {
            if remaining == 0 {
                break;
            }
            if deficits[i] > 0 {
                heights[i] = heights[i].saturating_add(1);
                deficits[i] -= 1;
                remaining -= 1;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }

    heights
}

fn render_board_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let tasks = tasks_filtered_base(app);
    let by_status = tasks_by_status(&tasks);
    let statuses = board_statuses(app);
    let needs: Vec<u16> = statuses
        .iter()
        .map(|status| {
            let list_len = match status {
                TaskStatus::Todo => by_status.todo.len(),
                TaskStatus::InProgress => by_status.inprogress.len(),
                TaskStatus::InReview => by_status.inreview.len(),
                TaskStatus::Done => by_status.done.len(),
                TaskStatus::Cancelled => by_status.cancelled.len(),
            };
            desired_board_section_height(list_len)
        })
        .collect();
    let heights = allocate_board_section_heights(&needs, area.height);
    let mut constraints: Vec<Constraint> = heights
        .iter()
        .copied()
        .map(|h| Constraint::Length(h))
        .collect();
    constraints.push(Constraint::Min(0)); // filler

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (idx, status) in statuses.iter().copied().enumerate() {
        if idx >= sections.len() {
            break;
        }

        let list: &[TaskRow] = match status {
            TaskStatus::Todo => &by_status.todo,
            TaskStatus::InProgress => &by_status.inprogress,
            TaskStatus::InReview => &by_status.inreview,
            TaskStatus::Done => &by_status.done,
            TaskStatus::Cancelled => &by_status.cancelled,
        };

        let is_active = status == app.tasks_active_column;
        let border_style = if app.focus == FocusPane::Board && is_active {
            Style::default().fg(Color::Cyan)
        } else if app.focus == FocusPane::Board {
            Style::default().fg(Color::Gray)
        } else {
            Style::default()
        };

        let title = format!("{} ({})", status.label(), list.len());

        let height = sections[idx].height.saturating_sub(2) as usize;
        let (items, selected_in_window) = if list.is_empty() || height == 0 {
            (vec![ListItem::new(Line::from("—"))], None)
        } else {
            let selected_idx = if is_active {
                task_index_in(list, app.selected_task_id)
                    .or_else(|| {
                        (!list.is_empty()).then_some(
                            app.board_index_by_status[status.idx()]
                                .min(list.len().saturating_sub(1)),
                        )
                    })
                    .unwrap_or(0)
            } else {
                0
            };
            let (start, end, selected_in_window) =
                window_for_list(list.len(), selected_idx, height);
            let visible = &list[start..end];
            let items = if visible.is_empty() {
                vec![ListItem::new(Line::from("—"))]
            } else {
                visible
                    .iter()
                    .map(|t| ListItem::new(render_task_line(t)))
                    .collect()
            };
            (items, is_active.then_some(selected_in_window))
        };

        let widget = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(border_style),
            )
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(if is_active { "▶ " } else { "  " });

        let mut state = ratatui::widgets::ListState::default();
        state.select(selected_in_window);
        f.render_stateful_widget(widget, sections[idx], &mut state);
    }
}

fn render_execution_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    render_logs_viewer(f, app, sections[0]);
    render_composer(f, app, sections[1]);
}

fn render_logs_viewer(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let border_style = if app.focus == FocusPane::Execution {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let err_line = app.last_error.as_ref().map(|e| {
        Line::from(vec![Span::styled(
            e.clone(),
            Style::default().fg(Color::Red),
        )])
    });
    let notice_lines = app.last_notice.as_ref().map(|m| {
        m.lines()
            .map(|line| {
                Line::from(vec![Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Green),
                )])
            })
            .collect::<Vec<_>>()
    });

    let len = app.log_lines.len();
    let max_render = area.height.saturating_sub(2) as usize;
    let visible = max_render.min(len);
    let mut offset = if app.log_autoscroll {
        0
    } else {
        app.log_scroll_offset
    };
    offset = offset.min(len.saturating_sub(visible));
    let start = len.saturating_sub(visible + offset);
    let end = len.saturating_sub(offset);

    let mut text: Vec<Line<'static>> = app.log_lines.get(start..end).unwrap_or(&[]).to_vec();
    if text.is_empty() {
        text.push(Line::from("No logs"));
    }
    if let Some(line) = err_line {
        text.push(Line::from(""));
        text.push(Line::from("Last error:"));
        text.push(line);
    }
    if let Some(lines) = notice_lines {
        text.push(Line::from(""));
        text.push(Line::from("Last notice:"));
        text.extend(lines);
    }

    let title = format!(
        "Run Logs ({}, {}, {})",
        match app.log_status {
            StreamStatus::Connected => "live",
            StreamStatus::Connecting => "connecting",
            StreamStatus::Completed => "done",
            StreamStatus::Disconnected => "offline",
            StreamStatus::Error => "error",
        },
        app.log_mode.label(),
        app.log_render_mode.label()
    );
    let w = Paragraph::new(text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );
    f.render_widget(w, area);
}

fn render_composer(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let border_style = if app.focus == FocusPane::Execution {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let hint = if app.composer_active {
        let prefix = "> ";
        let inner_w = area.width.saturating_sub(2) as usize;
        let prefix_w = display_width(prefix);
        let avail = inner_w.saturating_sub(prefix_w);

        let mut visible = app.composer_buffer.clone();
        if avail == 0 {
            visible.clear();
        } else if display_width(&visible) > avail {
            // Show the tail of the buffer (we only edit at the end for now).
            let mut out = String::new();
            for ch in visible.chars().rev() {
                if display_width(&out) >= avail.saturating_sub(1) {
                    break;
                }
                out.insert(0, ch);
            }
            visible = format!("…{out}");
        }

        let cursor_x = area
            .x
            .saturating_add(1)
            .saturating_add(prefix_w as u16)
            .saturating_add(display_width(&visible) as u16)
            .min(area.x.saturating_add(area.width).saturating_sub(1));
        let cursor_y = area.y.saturating_add(1);
        f.set_cursor_position((cursor_x, cursor_y));

        format!("{prefix}{visible}")
    } else {
        "Press i to type a follow-up or /command…".to_string()
    };

    let w = Paragraph::new(Line::from(hint))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input")
                .border_style(border_style),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(w, area);
}

#[derive(Debug, Clone)]
struct CompletionItem {
    insert: String,
    desc: String,
}

fn composer_is_slash_mode(s: &str) -> bool {
    s.trim_start().starts_with('/')
}

fn split_for_completion(s: &str) -> (Vec<&str>, &str, bool) {
    let trimmed = s.trim_start();
    let Some(rest) = trimmed.strip_prefix('/') else {
        return (vec![], "", true);
    };
    let ends_with_space = rest.chars().last().is_some_and(|c| c.is_whitespace());
    let mut tokens: Vec<&str> = rest.split_whitespace().collect();
    if ends_with_space {
        return (tokens, "", true);
    }
    let current = tokens.pop().unwrap_or("");
    (tokens, current, false)
}

fn composer_completion_items(app: &AppState) -> Vec<CompletionItem> {
    if !app.composer_active || !composer_is_slash_mode(&app.composer_buffer) {
        return vec![];
    }

    const COMMANDS: &[(&str, &str)] = &[
        ("help", "show help"),
        ("status", "refresh branch status"),
        ("repo", "select repo for git ops"),
        ("rebase", "rebase attempt branch"),
        ("abort", "abort conflicts/rebase"),
        ("merge", "squash-merge into target"),
        ("push", "push attempt branch"),
        ("pr", "PR actions"),
        ("open", "open file in editor"),
    ];

    const REBASE_FLAGS: &[(&str, &str)] = &[
        ("--onto", "new base branch"),
        ("--old", "old base branch"),
        ("--repo", "repo name or index"),
    ];
    const MERGE_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const PUSH_FLAGS: &[(&str, &str)] =
        &[("--force", "force push"), ("--repo", "repo name or index")];
    const ABORT_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const PR_SUB: &[(&str, &str)] = &[
        ("create", "create a PR"),
        ("attach", "attach existing PR"),
        ("comments", "fetch PR comments count"),
    ];
    const PR_CREATE_FLAGS: &[(&str, &str)] = &[
        ("--title", "PR title (required)"),
        ("--body", "PR body"),
        ("--base", "target branch"),
        ("--draft", "create as draft"),
        ("--auto-desc", "auto-generate description"),
        ("--repo", "repo name or index"),
    ];
    const PR_ATTACH_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];
    const PR_COMMENTS_FLAGS: &[(&str, &str)] = &[("--repo", "repo name or index")];

    let (tokens, current, ends_with_space) = split_for_completion(&app.composer_buffer);

    let current_lower = current.to_ascii_lowercase();
    let used_flags: std::collections::HashSet<&str> = tokens
        .iter()
        .copied()
        .filter(|t| t.starts_with("--"))
        .collect();

    let mut out: Vec<CompletionItem> = vec![];

    fn push_flags(
        out: &mut Vec<CompletionItem>,
        flags: &[(&'static str, &'static str)],
        used: &std::collections::HashSet<&str>,
        current_lower: &str,
        current_is_empty: bool,
    ) {
        for (flag, desc) in flags {
            if used.contains(*flag) {
                continue;
            }
            if current_is_empty || flag.starts_with(current_lower) {
                out.push(CompletionItem {
                    insert: format!("{flag} "),
                    desc: desc.to_string(),
                });
            }
        }
    }

    fn push_repos(
        out: &mut Vec<CompletionItem>,
        repos: &[RepoBranchStatus],
        current_lower: &str,
        current_is_empty: bool,
    ) {
        for (idx, r) in repos.iter().enumerate() {
            let name = r.repo_name.as_str();
            let name_l = name.to_ascii_lowercase();
            if current_is_empty
                || name_l.starts_with(current_lower)
                || name_l.contains(current_lower)
            {
                out.push(CompletionItem {
                    insert: format!("{name} "),
                    desc: "repo".to_string(),
                });
            }
            let n = format!("{}", idx + 1);
            if current_is_empty || n.starts_with(current_lower) {
                out.push(CompletionItem {
                    insert: format!("{n} "),
                    desc: "repo index".to_string(),
                });
            }
        }
    }

    if tokens.is_empty() {
        for (cmd, desc) in COMMANDS {
            if current.is_empty() || cmd.starts_with(&current_lower) {
                out.push(CompletionItem {
                    insert: format!("{cmd} "),
                    desc: (*desc).to_string(),
                });
            }
        }
        return out;
    }

    let cmd = tokens[0];
    if !COMMANDS.iter().any(|(c, _)| *c == cmd) {
        for (c, desc) in COMMANDS {
            if c.starts_with(&cmd.to_ascii_lowercase()) {
                out.push(CompletionItem {
                    insert: format!("{c} "),
                    desc: (*desc).to_string(),
                });
            }
        }
        return out;
    }

    let current_is_empty = current.is_empty();

    match cmd {
        "repo" => {
            push_repos(
                &mut out,
                &app.repo_statuses,
                &current_lower,
                current_is_empty,
            );
        }
        "rebase" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(
                    &mut out,
                    REBASE_FLAGS,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(
                    &mut out,
                    &app.repo_statuses,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        "abort" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(
                    &mut out,
                    ABORT_FLAGS,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(
                    &mut out,
                    &app.repo_statuses,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        "merge" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(
                    &mut out,
                    MERGE_FLAGS,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(
                    &mut out,
                    &app.repo_statuses,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        "push" => {
            if current.starts_with("--") || ends_with_space {
                push_flags(
                    &mut out,
                    PUSH_FLAGS,
                    &used_flags,
                    &current_lower,
                    current_is_empty,
                );
            } else if tokens.last().is_some_and(|t| *t == "--repo") {
                push_repos(
                    &mut out,
                    &app.repo_statuses,
                    &current_lower,
                    current_is_empty,
                );
            }
        }
        "pr" => {
            if tokens.len() == 1 {
                for (sub, desc) in PR_SUB {
                    if current.is_empty() || sub.starts_with(&current_lower) {
                        out.push(CompletionItem {
                            insert: format!("{sub} "),
                            desc: (*desc).to_string(),
                        });
                    }
                }
            } else {
                let sub = tokens.get(1).copied().unwrap_or("");
                match sub {
                    "create" => {
                        if current.starts_with("--") || ends_with_space {
                            push_flags(
                                &mut out,
                                PR_CREATE_FLAGS,
                                &used_flags,
                                &current_lower,
                                current_is_empty,
                            );
                        } else if tokens.last().is_some_and(|t| *t == "--repo") {
                            push_repos(
                                &mut out,
                                &app.repo_statuses,
                                &current_lower,
                                current_is_empty,
                            );
                        }
                    }
                    "attach" => {
                        if current.starts_with("--") || ends_with_space {
                            push_flags(
                                &mut out,
                                PR_ATTACH_FLAGS,
                                &used_flags,
                                &current_lower,
                                current_is_empty,
                            );
                        } else if tokens.last().is_some_and(|t| *t == "--repo") {
                            push_repos(
                                &mut out,
                                &app.repo_statuses,
                                &current_lower,
                                current_is_empty,
                            );
                        }
                    }
                    "comments" => {
                        if current.starts_with("--") || ends_with_space {
                            push_flags(
                                &mut out,
                                PR_COMMENTS_FLAGS,
                                &used_flags,
                                &current_lower,
                                current_is_empty,
                            );
                        } else if tokens.last().is_some_and(|t| *t == "--repo") {
                            push_repos(
                                &mut out,
                                &app.repo_statuses,
                                &current_lower,
                                current_is_empty,
                            );
                        }
                    }
                    _ => {
                        for (sub, desc) in PR_SUB {
                            out.push(CompletionItem {
                                insert: format!("{sub} "),
                                desc: (*desc).to_string(),
                            });
                        }
                    }
                }
            }
        }
        "open" | "status" | "help" => {}
        _ => {}
    }

    out
}

fn move_composer_autocomplete(app: &mut AppState, delta: i32) {
    if !composer_is_slash_mode(&app.composer_buffer) {
        return;
    }
    let items = composer_completion_items(app);
    if items.is_empty() {
        return;
    }
    let len = items.len();
    let cur = app.composer_suggest_index.min(len - 1);
    let next = clamp_index(cur, delta, len);
    app.composer_suggest_index = next;
}

fn apply_composer_autocomplete(app: &mut AppState) -> bool {
    if !composer_is_slash_mode(&app.composer_buffer) {
        return false;
    }

    let items = composer_completion_items(app);
    if items.is_empty() {
        return false;
    }

    let idx = app.composer_suggest_index.min(items.len() - 1);
    let insert = items[idx].insert.as_str();

    let buf = app.composer_buffer.clone();
    let mut token_start = buf
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);

    if let Some(slash_pos) = buf.find('/') {
        if token_start <= slash_pos {
            token_start = slash_pos + 1;
        }
    }

    app.composer_buffer.truncate(token_start);
    app.composer_buffer.push_str(insert);
    app.composer_suggest_index = 0;
    true
}

fn render_composer_autocomplete(f: &mut Frame, app: &AppState, input_area: ratatui::layout::Rect) {
    if !app.composer_active || !composer_is_slash_mode(&app.composer_buffer) {
        return;
    }

    let items = composer_completion_items(app);
    if items.is_empty() {
        return;
    }

    let max_items = 6usize;
    let visible = items.len().min(max_items);
    let height = (visible + 2).min(input_area.y as usize);
    if height < 3 {
        return;
    }
    let height_u16 = height as u16;
    let y = input_area.y.saturating_sub(height_u16);
    let area = ratatui::layout::Rect {
        x: input_area.x,
        y,
        width: input_area.width,
        height: height_u16,
    };

    f.render_widget(Clear, area);

    let start = app
        .composer_suggest_index
        .saturating_sub(visible.saturating_sub(1));
    let end = (start + visible).min(items.len());
    let window = &items[start..end];

    let list_items: Vec<ListItem> = window
        .iter()
        .cloned()
        .map(|it| {
            let mut spans: Vec<Span<'static>> = vec![Span::styled(
                it.insert.trim().to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            )];
            if !it.desc.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    it.desc,
                    Style::default().add_modifier(Modifier::DIM),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut state = ratatui::widgets::ListState::default();
    let selected_in_window = app.composer_suggest_index.saturating_sub(start);
    state.select(Some(
        selected_in_window.min(list_items.len().saturating_sub(1)),
    ));

    let w = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Commands")
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");

    f.render_stateful_widget(w, area, &mut state);
}

#[derive(Debug, Clone)]
struct DiffRow {
    key: String,
    change: Option<String>,
    additions: Option<usize>,
    deletions: Option<usize>,
    content_omitted: bool,
    old_path: Option<String>,
    new_path: Option<String>,
}

const DIFF_ALL_KEY: &str = "__ALL__";

fn diff_rows(store: &serde_json::Value) -> Vec<DiffRow> {
    let Some(entries) = store.get("entries").and_then(|v| v.as_object()) else {
        return vec![];
    };

    let mut rows = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        if value.get("type").and_then(|v| v.as_str()) != Some("DIFF") {
            continue;
        }
        let Some(content) = value.get("content") else {
            continue;
        };

        let change = content
            .get("change")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let additions = content
            .get("additions")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let deletions = content
            .get("deletions")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let content_omitted = content
            .get("contentOmitted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let old_path = content
            .get("oldPath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let new_path = content
            .get("newPath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        rows.push(DiffRow {
            key: key.clone(),
            change,
            additions,
            deletions,
            content_omitted,
            old_path,
            new_path,
        });
    }

    rows.sort_by(|a, b| a.key.cmp(&b.key));
    rows
}

fn diff_rows_with_all(store: &serde_json::Value) -> Vec<DiffRow> {
    let mut rows = diff_rows(store);
    if rows.is_empty() {
        return rows;
    }

    let mut total_adds = 0usize;
    let mut total_dels = 0usize;
    let mut any_omitted = false;
    for row in &rows {
        total_adds = total_adds.saturating_add(row.additions.unwrap_or(0));
        total_dels = total_dels.saturating_add(row.deletions.unwrap_or(0));
        any_omitted |= row.content_omitted;
    }

    rows.insert(
        0,
        DiffRow {
            key: DIFF_ALL_KEY.to_string(),
            change: Some("all".to_string()),
            additions: Some(total_adds),
            deletions: Some(total_dels),
            content_omitted: any_omitted,
            old_path: None,
            new_path: None,
        },
    );
    rows
}

fn render_diff_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Min(3),
        ])
        .split(area);

    render_diff_repo_bar(f, app, sections[0]);
    render_diff_files(f, app, sections[1]);
    render_diff_preview(f, app, sections[2]);
}

fn selected_attempt_branch(app: &AppState) -> String {
    app.selected_attempt_id
        .and_then(|id| app.attempts.iter().find(|a| a.id == id))
        .map(|a| a.branch.clone())
        .unwrap_or_else(|| "—".to_string())
}

fn repo_name_from_path(path: &str) -> Option<&str> {
    let first = path.split('/').next()?;
    if first.is_empty() { None } else { Some(first) }
}

fn selected_repo_status_from_diff(app: &AppState) -> Option<usize> {
    if app.repo_statuses.is_empty() {
        return None;
    }
    let rows = diff_rows_with_all(&app.diff_store);
    let selected = rows.get(app.selected_diff_index)?;
    let path = selected
        .new_path
        .as_deref()
        .or(selected.old_path.as_deref())
        .unwrap_or(&selected.key);
    let repo = repo_name_from_path(path)?;
    app.repo_statuses.iter().position(|r| r.repo_name == repo)
}

fn sync_selected_repo_from_diff_selection(app: &mut AppState) {
    if let Some(idx) = selected_repo_status_from_diff(app) {
        app.selected_repo_index = idx;
    }
}

fn badge(text: impl Into<String>, fg: Color, bg: Color) -> Span<'static> {
    Span::styled(
        format!(" {} ", text.into()),
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
    )
}

#[derive(Debug, Clone, Copy)]
enum DiffRepoAction {
    Merge,
    CreatePr,
    Rebase,
    RefreshStatus,
}

fn diff_repo_bar_action_at(
    app: &AppState,
    area: ratatui::layout::Rect,
    col: u16,
    row: u16,
) -> Option<DiffRepoAction> {
    if !rect_contains(area, col, row) {
        return None;
    }

    let inner_x0 = area.x.saturating_add(1);
    let inner_x1 = area.x.saturating_add(area.width).saturating_sub(1);
    let inner_y0 = area.y.saturating_add(1);
    let inner_y1 = area.y.saturating_add(area.height).saturating_sub(1);
    if col < inner_x0 || col >= inner_x1 || row < inner_y0 || row >= inner_y1 {
        return None;
    }

    let w = area.width.saturating_sub(2) as usize;
    let repo = app.repo_statuses.get(
        app.selected_repo_index
            .min(app.repo_statuses.len().saturating_sub(1)),
    );
    let branch = selected_attempt_branch(app);

    let (repo_name, target_branch, ahead, behind, conflicts, pr_open) = if let Some(r) = repo {
        let ahead = r.status.commits_ahead.unwrap_or(0);
        let behind = r.status.commits_behind.unwrap_or(0);
        let conflicts = r.status.conflicted_files.len();
        let pr_open = r.status.merges.iter().find_map(|m| match m {
            Merge::Pr(pr) => Some(pr.pr_info.number),
            _ => None,
        });
        (
            r.repo_name.clone(),
            r.status.target_branch_name.clone(),
            ahead,
            behind,
            conflicts,
            pr_open,
        )
    } else {
        ("(repo)".to_string(), "—".to_string(), 0, 0, 0, None)
    };

    let left_base = if repo.is_some() {
        format!("{repo_name}  {branch} → {target_branch}")
    } else if app.selected_attempt_id.is_some() {
        format!("{branch}  (press S for repo status)")
    } else {
        "(no attempt)".to_string()
    };

    let mut right_plain = String::new();
    let mut any_badge = false;
    if ahead > 0 {
        right_plain.push_str(&format!(" +{ahead} "));
        any_badge = true;
    }
    if behind > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" {behind} "));
        any_badge = true;
    }
    if conflicts > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" !{conflicts} "));
        any_badge = true;
    }
    if let Some(n) = pr_open {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" PR#{n} "));
        any_badge = true;
    }
    if any_badge {
        right_plain.push_str("  ");
    }
    right_plain.push_str("[M]erge [P]R [R]ebase [S]tatus");
    let right_w = display_width(&right_plain);

    let can_show_right = w > right_w + 2;
    if !can_show_right {
        return None;
    }

    let left_w = w - right_w - 2;
    let left = truncate_to_width(&left_base, left_w);

    let inner_col = col.saturating_sub(inner_x0) as usize;
    let mut cursor = display_width(&left);
    cursor = cursor.saturating_add(2); // after "  "

    let mut first_badge = true;
    let mut push_badge = |label: String| {
        if !first_badge {
            cursor = cursor.saturating_add(1);
        }
        first_badge = false;
        cursor = cursor.saturating_add(display_width(&format!(" {label} ")));
    };
    if ahead > 0 {
        push_badge(format!("+{ahead}"));
    }
    if behind > 0 {
        push_badge(format!("{behind}"));
    }
    if conflicts > 0 {
        push_badge(format!("!{conflicts}"));
    }
    if let Some(n) = pr_open {
        push_badge(format!("PR#{n}"));
    }
    if !first_badge {
        cursor = cursor.saturating_add(2); // before buttons
    }

    let buttons: [(&str, DiffRepoAction); 4] = [
        ("[M]erge", DiffRepoAction::Merge),
        ("[P]R", DiffRepoAction::CreatePr),
        ("[R]ebase", DiffRepoAction::Rebase),
        ("[S]tatus", DiffRepoAction::RefreshStatus),
    ];
    for (idx, (label, action)) in buttons.iter().enumerate() {
        let start = cursor;
        let end = start.saturating_add(display_width(label));
        if inner_col >= start && inner_col < end {
            return Some(*action);
        }
        cursor = end;
        if idx + 1 < buttons.len() {
            cursor = cursor.saturating_add(1);
        }
    }

    None
}

fn trigger_diff_repo_action(app: &mut AppState, action: DiffRepoAction) {
    match action {
        DiffRepoAction::RefreshStatus => {
            request_branch_status_refresh(app);
        }
        DiffRepoAction::Merge => {
            let Ok((repo_id, repo_name)) = resolve_repo_for_command(app, None) else {
                return;
            };
            let Some(attempt_id) = app.selected_attempt_id else {
                return;
            };
            let base_url = app.backend_url.clone();
            let net_tx = app.net_tx.clone();
            tokio::spawn(async move {
                match merge_task_attempt_http(&base_url, attempt_id, repo_id).await {
                    Ok(()) => {
                        let _ = net_tx
                            .send(NetEvent::Notice(format!("Merged {repo_name}.")))
                            .await;
                        if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                            let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                        }
                    }
                    Err(e) => {
                        let _ = net_tx
                            .send(NetEvent::Error(format!("merge failed: {e}")))
                            .await;
                    }
                }
            });
        }
        DiffRepoAction::Rebase => {
            let Ok((repo_id, repo_name)) = resolve_repo_for_command(app, None) else {
                return;
            };
            let Some(attempt_id) = app.selected_attempt_id else {
                return;
            };
            let base_url = app.backend_url.clone();
            let net_tx = app.net_tx.clone();
            tokio::spawn(async move {
                match rebase_task_attempt_http(&base_url, attempt_id, repo_id, None, None).await {
                    Ok(()) => {
                        let _ = net_tx
                            .send(NetEvent::Notice(format!("Rebase started for {repo_name}.")))
                            .await;
                        if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                            let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                        }
                    }
                    Err(e) => {
                        let _ = net_tx
                            .send(NetEvent::Error(format!("rebase failed: {e}")))
                            .await;
                    }
                }
            });
        }
        DiffRepoAction::CreatePr => {
            let Ok((repo_id, repo_name)) = resolve_repo_for_command(app, None) else {
                return;
            };
            let Some(attempt_id) = app.selected_attempt_id else {
                return;
            };
            let title = app
                .selected_task_id
                .and_then(|id| find_task(&app.tasks_store, id).map(|t| t.title))
                .unwrap_or_else(|| "Vibe Kanban PR".to_string());
            let base_url = app.backend_url.clone();
            let net_tx = app.net_tx.clone();
            tokio::spawn(async move {
                match create_pr_http(
                    &base_url,
                    attempt_id,
                    CreateGitHubPrRequest {
                        title,
                        body: None,
                        target_branch: None,
                        draft: Some(false),
                        repo_id,
                        auto_generate_description: false,
                    },
                )
                .await
                {
                    Ok(url) => {
                        let _ = net_tx
                            .send(NetEvent::Notice(format!(
                                "PR created for {repo_name}: {url}"
                            )))
                            .await;
                        if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                            let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                        }
                    }
                    Err(e) => {
                        let _ = net_tx
                            .send(NetEvent::Error(format!("pr create failed: {e}")))
                            .await;
                    }
                }
            });
        }
    }
}

fn render_diff_repo_bar(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let border_style = if app.focus == FocusPane::Diff {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let w = area.width.saturating_sub(2) as usize;

    let repo = app.repo_statuses.get(
        app.selected_repo_index
            .min(app.repo_statuses.len().saturating_sub(1)),
    );
    let branch = selected_attempt_branch(app);

    let (repo_name, target_branch, ahead, behind, conflicts, pr_open) = if let Some(r) = repo {
        let ahead = r.status.commits_ahead.unwrap_or(0);
        let behind = r.status.commits_behind.unwrap_or(0);
        let conflicts = r.status.conflicted_files.len();
        let pr_open = r.status.merges.iter().find_map(|m| match m {
            Merge::Pr(pr) => Some(pr.pr_info.number),
            _ => None,
        });
        (
            r.repo_name.clone(),
            r.status.target_branch_name.clone(),
            ahead,
            behind,
            conflicts,
            pr_open,
        )
    } else {
        ("(repo)".to_string(), "—".to_string(), 0, 0, 0, None)
    };

    let left_base = if repo.is_some() {
        format!("{repo_name}  {branch} → {target_branch}")
    } else if app.selected_attempt_id.is_some() {
        format!("{branch}  (press S for repo status)")
    } else {
        "(no attempt)".to_string()
    };

    // Compute right-side width based on what we actually render (badges + buttons),
    // so we don't truncate the left segment unnecessarily.
    let mut right_plain = String::new();
    let mut any_badge = false;
    if ahead > 0 {
        right_plain.push_str(&format!(" +{ahead} "));
        any_badge = true;
    }
    if behind > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" {behind} "));
        any_badge = true;
    }
    if conflicts > 0 {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" !{conflicts} "));
        any_badge = true;
    }
    if let Some(n) = pr_open {
        if any_badge {
            right_plain.push(' ');
        }
        right_plain.push_str(&format!(" PR#{n} "));
        any_badge = true;
    }
    if any_badge {
        right_plain.push_str("  ");
    }
    right_plain.push_str("[M]erge [P]R [R]ebase [S]tatus");
    let right_w = display_width(&right_plain);

    let can_show_right = w > right_w + 2;
    let left_w = if can_show_right { w - right_w - 2 } else { w };
    let left = truncate_to_width(&left_base, left_w);

    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        left,
        Style::default().add_modifier(Modifier::BOLD),
    )];

    if can_show_right {
        spans.push(Span::raw("  "));
        // Render badges + buttons with colors.
        let mut first = true;
        if ahead > 0 {
            spans.push(badge(format!("+{ahead}"), Color::Black, Color::LightGreen));
            first = false;
        }
        if behind > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            spans.push(badge(format!("{behind}"), Color::Black, Color::LightYellow));
            first = false;
        }
        if conflicts > 0 {
            if !first {
                spans.push(Span::raw(" "));
            }
            spans.push(badge(format!("!{conflicts}"), Color::White, Color::Red));
            first = false;
        }
        if let Some(n) = pr_open {
            if !first {
                spans.push(Span::raw(" "));
            }
            spans.push(badge(format!("PR#{n}"), Color::Black, Color::LightBlue));
            first = false;
        }

        if !first {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            "[M]erge".to_string(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "[P]R".to_string(),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "[R]ebase".to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "[S]tatus".to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Repo")
            .border_style(border_style),
    );
    f.render_widget(p, area);
}

fn render_diff_files(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let rows = diff_rows_with_all(&app.diff_store);
    let file_count = rows.len().saturating_sub(1);
    let border_style = if app.focus == FocusPane::Diff && app.diff_focus == DiffFocus::Files {
        Style::default().fg(Color::Cyan)
    } else if app.focus == FocusPane::Diff {
        Style::default()
    } else {
        Style::default()
    };

    let title = format!(
        "Files ({}, {})",
        file_count,
        match app.diff_status {
            StreamStatus::Connected => "live",
            StreamStatus::Connecting => "connecting",
            StreamStatus::Completed => "done",
            StreamStatus::Disconnected => "offline",
            StreamStatus::Error => "error",
        }
    );

    let selected = if rows.is_empty() {
        0
    } else {
        app.selected_diff_index.min(rows.len() - 1)
    };

    let height = area.height.saturating_sub(2) as usize;
    let (start, end, selected_in_window) = window_for_list(rows.len(), selected, height);
    let visible = &rows[start..end];

    let items: Vec<ListItem> = if visible.is_empty() {
        vec![ListItem::new(Line::from("No diffs"))]
    } else {
        visible
            .iter()
            .map(|d| {
                const LABEL_W: usize = 8;

                fn label_spans(label: &str, style: Style) -> Vec<Span<'static>> {
                    const LABEL_W: usize = 8;
                    let pad = LABEL_W.saturating_sub(label.len());
                    vec![
                        Span::raw(" ".repeat(pad)),
                        Span::styled(label.to_string(), style),
                        Span::raw(" "),
                    ]
                }

                if d.key == DIFF_ALL_KEY {
                    let mut spans: Vec<Span<'static>> = vec![];
                    spans.extend(label_spans(
                        "ALL",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(
                        "All changes".to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));

                    if d.content_omitted {
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            "[omitted]".to_string(),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }

                    if let (Some(adds), Some(dels)) = (d.additions, d.deletions) {
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            format!("+{adds}"),
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::raw("/"));
                        spans.push(Span::styled(
                            format!("-{dels}"),
                            Style::default()
                                .fg(Color::White)
                                .bg(Color::Red)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }

                    return ListItem::new(Line::from(spans));
                }

                let change = d.change.as_deref().unwrap_or("unknown");
                let (label, change_style) = match change {
                    "added" | "Added" => ("ADD", Style::default().fg(Color::Green)),
                    "deleted" | "Deleted" => ("DEL", Style::default().fg(Color::Red)),
                    "modified" | "Modified" => ("MOD", Style::default().fg(Color::Yellow)),
                    "renamed" | "Renamed" => ("REN", Style::default().fg(Color::Cyan)),
                    "copied" | "Copied" => ("CPY", Style::default().fg(Color::Blue)),
                    "permission_change" | "PermissionChange" | "Permission Change" => {
                        ("CHMOD", Style::default().fg(Color::Magenta))
                    }
                    _ => ("?", Style::default().add_modifier(Modifier::DIM)),
                };

                let path_display = if label == "REN" {
                    match (d.old_path.as_deref(), d.new_path.as_deref()) {
                        (Some(old), Some(new)) if !old.is_empty() && !new.is_empty() => {
                            format!("{old} → {new}")
                        }
                        _ => d.key.clone(),
                    }
                } else {
                    d.key.clone()
                };

                let (dir_part, base_part) = match path_display.rsplit_once('/') {
                    Some((dir, base)) if !dir.is_empty() => {
                        (Some(dir.to_string()), base.to_string())
                    }
                    _ => (None, path_display),
                };

                let mut spans: Vec<Span<'static>> = vec![];
                let styled_label = if label.len() <= LABEL_W && label != "?" {
                    change_style.add_modifier(Modifier::BOLD)
                } else {
                    change_style
                };
                spans.extend(label_spans(label, styled_label));

                if let Some(dir) = dir_part {
                    spans.push(Span::styled(
                        format!("{dir}/"),
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
                spans.push(Span::styled(
                    base_part,
                    Style::default().add_modifier(Modifier::BOLD),
                ));

                if d.content_omitted {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        "[omitted]",
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }

                if let (Some(a), Some(b)) = (d.additions, d.deletions) {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("+{a}"),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::raw("/"));
                    spans.push(Span::styled(
                        format!("-{b}"),
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if let Some(a) = d.additions {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("+{a}"),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if let Some(b) = d.deletions {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("-{b}"),
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect()
    };

    let widget = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(
            if app.focus == FocusPane::Diff && app.diff_focus == DiffFocus::Files {
                "▶ "
            } else {
                "  "
            },
        );

    let mut state = ratatui::widgets::ListState::default();
    if !visible.is_empty() {
        state.select(Some(selected_in_window));
    }
    f.render_stateful_widget(widget, area, &mut state);
}

fn render_diff_preview(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let border_style = if app.focus == FocusPane::Diff && app.diff_focus == DiffFocus::Preview {
        Style::default().fg(Color::Cyan)
    } else if app.focus == FocusPane::Diff {
        Style::default()
    } else {
        Style::default()
    };

    let lines = &app.diff_preview_lines;
    let start = app.diff_scroll_offset.min(lines.len());
    let height = area.height.saturating_sub(2) as usize;
    let end = (start + height).min(lines.len());
    let visible = lines.get(start..end).unwrap_or(&[]);

    let w = Paragraph::new(visible.to_vec()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                "Diff ({}){}",
                app.diff_theme.label(),
                if app.diff_wrap { ", wrap" } else { "" }
            ))
            .border_style(border_style),
    );

    f.render_widget(w, area);
}

fn render_projects_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let border_style = Style::default();

    let projects = filtered_projects(app);
    let items: Vec<ListItem> = if projects.is_empty() {
        vec![ListItem::new(Line::from("No projects"))]
    } else {
        projects
            .iter()
            .map(|p| ListItem::new(Line::from(p.name.clone())))
            .collect()
    };

    let title = format!(
        "Projects ({}){}",
        match app.projects_status {
            StreamStatus::Connected => "live",
            StreamStatus::Connecting => "connecting",
            StreamStatus::Completed => "done",
            StreamStatus::Disconnected => "offline",
            StreamStatus::Error => "error",
        },
        if app.project_filter.trim().is_empty() {
            "".to_string()
        } else {
            format!(" /{}", app.project_filter.trim())
        }
    );

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");

    let mut state = ratatui::widgets::ListState::default();
    if !projects.is_empty() {
        let idx = app
            .selected_project_id
            .and_then(|id| projects.iter().position(|p| p.id == id))
            .unwrap_or(0);
        state.select(Some(idx));
    }
    f.render_stateful_widget(list, area, &mut state);
}

fn render_tasks_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    render_tasks_board(f, app, area)
}

fn render_tasks_board(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let tasks = tasks_filtered_base(app);
    let by_status = tasks_by_status(&tasks);
    let columns: Vec<TaskStatus> = if app.show_cancelled {
        vec![
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::InReview,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ]
    } else {
        vec![
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::InReview,
            TaskStatus::Done,
        ]
    };

    let pct = (100 / columns.len().max(1)) as u16;
    let constraints: Vec<Constraint> = (0..columns.len())
        .map(|_| Constraint::Percentage(pct))
        .collect();

    let col_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, status) in columns.into_iter().enumerate() {
        let list: &[TaskRow] = match status {
            TaskStatus::Todo => &by_status.todo,
            TaskStatus::InProgress => &by_status.inprogress,
            TaskStatus::InReview => &by_status.inreview,
            TaskStatus::Done => &by_status.done,
            TaskStatus::Cancelled => &by_status.cancelled,
        };

        let is_active_col = app.focus == FocusPane::Board && app.tasks_active_column == status;
        let border_style = if is_active_col {
            Style::default().fg(Color::Cyan)
        } else if app.focus == FocusPane::Board {
            Style::default()
        } else {
            Style::default()
        };

        let title = format!("{} ({})", status.label(), list.len());
        let items: Vec<ListItem> = if list.is_empty() {
            vec![ListItem::new(Line::from("—"))]
        } else {
            list.iter()
                .map(|t| ListItem::new(render_task_line(t)))
                .collect()
        };

        let widget = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(border_style),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▶ ");

        let mut state = ratatui::widgets::ListState::default();
        if let Some(idx) = task_index_in(list, app.selected_task_id) {
            state.select(Some(idx));
        } else if is_active_col && !list.is_empty() {
            state.select(Some(0));
        }

        f.render_stateful_widget(widget, col_areas[i], &mut state);
    }
}

fn render_tasks_table(f: &mut Frame, _app: &AppState, area: ratatui::layout::Rect) {
    let w = Paragraph::new("Table view removed (use the board)")
        .block(Block::default().borders(Borders::ALL).title("Tasks"));
    f.render_widget(w, area);
}

fn render_details_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Min(5),
            Constraint::Min(5),
        ])
        .split(area);

    let project_name = app.selected_project_id.and_then(|id| {
        projects_list(&app.projects_store)
            .into_iter()
            .find(|p| p.id == id)
            .map(|p| p.name)
    });

    let task = app
        .selected_task_id
        .and_then(|id| find_task(&app.tasks_store, id));

    let mut header_lines = vec![
        Line::from(vec![Span::styled(
            "Details",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!(
            "project: {}",
            project_name.unwrap_or_else(|| "none".to_string())
        )),
    ];
    match task.as_ref() {
        Some(t) => {
            header_lines.push(Line::from(format!("task: {}", t.title)));
            header_lines.push(Line::from(format!("status: {}", t.status.as_api_str())));
            if let Some(desc) = t.description.as_ref().filter(|s| !s.trim().is_empty()) {
                let first = desc.lines().next().unwrap_or("").trim();
                if !first.is_empty() {
                    header_lines.push(Line::from(format!("desc: {}", truncate(first, 60))));
                }
            }
        }
        None => header_lines.push(Line::from("task: none")),
    }
    header_lines.push(Line::from(app.info_summary.clone()));

    let header = Paragraph::new(header_lines)
        .block(Block::default().borders(Borders::ALL).title("Task"))
        .wrap(Wrap { trim: true });
    f.render_widget(header, sections[0]);

    let attempts_border = Style::default();

    let attempts_items: Vec<ListItem> = if app.attempts.is_empty() {
        vec![ListItem::new(Line::from("No attempts"))]
    } else {
        app.attempts
            .iter()
            .map(|a| {
                let when = a.created_at.as_deref().and_then(short_time).unwrap_or("");
                let updated = a.updated_at.as_deref().and_then(short_time).unwrap_or("");
                let suffix = if a.setup_completed_at.is_some() {
                    " setup✓"
                } else {
                    ""
                };
                let text = if when.is_empty() && updated.is_empty() {
                    format!("{}{}", a.branch, suffix)
                } else if updated.is_empty() {
                    format!("{} [{}]{}", a.branch, when, suffix)
                } else if when.is_empty() {
                    format!("{} [u:{}]{}", a.branch, updated, suffix)
                } else {
                    format!("{} [{} u:{}]{}", a.branch, when, updated, suffix)
                };
                ListItem::new(Line::from(text))
            })
            .collect()
    };
    let attempts_title = format!("Attempts ({})", app.attempts.len());
    let attempts_list = List::new(attempts_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(attempts_title)
                .border_style(attempts_border),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    let mut attempts_state = ratatui::widgets::ListState::default();
    if !app.attempts.is_empty() {
        attempts_state.select(Some(app.selected_attempt_index.min(app.attempts.len() - 1)));
    }
    f.render_stateful_widget(attempts_list, sections[1], &mut attempts_state);

    let execs_border = Style::default();

    let execs = exec_list(&app.exec_store);
    let exec_items: Vec<ListItem> = if execs.is_empty() {
        vec![ListItem::new(Line::from("No execution processes"))]
    } else {
        execs
            .iter()
            .map(|e| {
                let reason = e.run_reason.clone().unwrap_or_else(|| "?".to_string());
                let status = e.status.clone().unwrap_or_else(|| "?".to_string());
                let dropped = if e.dropped { " dropped" } else { "" };
                ListItem::new(Line::from(format!("{reason}: {status}{dropped}")))
            })
            .collect()
    };
    let exec_title = format!(
        "Execs ({}) {}",
        execs.len(),
        match app.exec_status {
            StreamStatus::Connected => "live",
            StreamStatus::Connecting => "connecting",
            StreamStatus::Completed => "done",
            StreamStatus::Disconnected => "offline",
            StreamStatus::Error => "error",
        }
    );
    let exec_list_widget = List::new(exec_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(exec_title)
                .border_style(execs_border),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    let mut exec_state = ratatui::widgets::ListState::default();
    if !execs.is_empty() {
        let idx = app
            .selected_exec_id
            .and_then(|id| execs.iter().position(|e| e.id == id))
            .unwrap_or(0);
        exec_state.select(Some(idx));
    }
    f.render_stateful_widget(exec_list_widget, sections[2], &mut exec_state);
}

fn render_logs_pane(f: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let border_style = if app.focus == FocusPane::Execution {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let err_line = app.last_error.as_ref().map(|e| {
        Line::from(vec![Span::styled(
            e.clone(),
            Style::default().fg(Color::Red),
        )])
    });

    let max_render = 200usize;
    let len = app.log_lines.len();
    let visible = max_render.min(len);
    let mut offset = if app.log_autoscroll {
        0
    } else {
        app.log_scroll_offset
    };
    offset = offset.min(len.saturating_sub(visible));
    let start = len.saturating_sub(visible + offset);
    let end = len.saturating_sub(offset);

    let mut text: Vec<Line<'static>> = app.log_lines.get(start..end).unwrap_or(&[]).to_vec();
    if text.is_empty() {
        text.push(Line::from("No logs"));
    }
    if let Some(line) = err_line {
        text.push(Line::from(""));
        text.push(Line::from("Last error:"));
        text.push(line);
    }

    let title = format!(
        "Logs ({}, {}, {})",
        match app.log_status {
            StreamStatus::Connected => "live",
            StreamStatus::Connecting => "connecting",
            StreamStatus::Completed => "done",
            StreamStatus::Disconnected => "offline",
            StreamStatus::Error => "error",
        },
        app.log_mode.label(),
        app.log_render_mode.label()
    );
    let p = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn render_help_modal(f: &mut Frame) {
    let area = centered_rect(70, 70, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(vec![Span::styled(
            "Vibe Kanban TUI — Help",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Global"),
        Line::from("  q           quit"),
        Line::from("  Tab         cycle focus"),
        Line::from("  /           search tasks"),
        Line::from("  r           reconnect streams"),
        Line::from("  ? / Esc     close help"),
        Line::from(""),
        Line::from("Board (left)"),
        Line::from("  j/k or ↑/↓  move within status"),
        Line::from("  J/K         change status section"),
        Line::from("  ←/→         move task status"),
        Line::from("  [ / ]       switch attempt"),
        Line::from("  c           toggle cancelled section"),
        Line::from(""),
        Line::from("Execution (center)"),
        Line::from("  i           compose follow-up"),
        Line::from("  Enter       send follow-up (while composing)"),
        Line::from("  /<cmd>      run slash command (while composing)"),
        Line::from("  Tab         autocomplete (slash mode)"),
        Line::from("  ↑/↓         select suggestion (slash mode)"),
        Line::from("  e / Enter   expand/collapse entry"),
        Line::from("  Esc         cancel compose"),
        Line::from("  o           toggle raw/normalized"),
        Line::from("  PgUp/PgDn   scroll logs"),
        Line::from("  End         jump bottom"),
        Line::from("  x           stop active run"),
        Line::from(""),
        Line::from("Slash commands"),
        Line::from("  /status                 refresh repo branch status"),
        Line::from("  /repo [name|n]           select repo for git ops"),
        Line::from("  /rebase [--onto B]       rebase attempt branch"),
        Line::from("  /merge                   squash-merge into target"),
        Line::from("  /push [--force]          push branch"),
        Line::from("  /abort                   abort conflicts/rebase"),
        Line::from("  /pr create --title T     create PR (server-side)"),
        Line::from("  /pr attach               attach existing PR"),
        Line::from("  /open <file>             open file in editor"),
        Line::from(""),
        Line::from("Diff (right)"),
        Line::from("  j/k         select file"),
        Line::from("  h/l         files/preview focus"),
        Line::from("  PgUp/PgDn   scroll diff preview"),
        Line::from("  d           toggle stats-only"),
        Line::from("  t           cycle theme"),
        Line::from("  M           merge (selected repo)"),
        Line::from("  P           create PR (selected repo)"),
        Line::from("  R           rebase (selected repo)"),
        Line::from("  S           refresh branch status"),
    ];

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn render_confirm_modal(f: &mut Frame, confirm: &ConfirmState) {
    let area = centered_rect(70, 35, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(vec![Span::styled(
            confirm.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(confirm.body.clone()),
        Line::from(""),
        Line::from("y = confirm, n/Esc = cancel"),
    ];

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Confirm"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn render_input_modal(f: &mut Frame, input: &InputState) {
    let area = centered_rect(80, 25, f.area());
    f.render_widget(Clear, area);

    let (title, hint) = match input.mode {
        InputMode::SearchTasks => (
            "Search tasks",
            "type to filter, Enter to apply, Esc to cancel",
        ),
    };

    let lines = vec![
        Line::from(vec![Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(format!("/{}", input.buffer)),
        Line::from(""),
        Line::from(hint),
    ];

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn prefs_path() -> PathBuf {
    utils::assets::asset_dir().join("tui.json")
}

fn load_prefs() -> TuiPrefs {
    let path = prefs_path();
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => TuiPrefs::default(),
    }
}

fn save_prefs(prefs: &TuiPrefs) {
    let path = prefs_path();
    match serde_json::to_string_pretty(prefs) {
        Ok(raw) => {
            let _ = fs::write(path, raw);
        }
        Err(_) => {}
    }
}

async fn resolve_backend_url(args: &Args) -> anyhow::Result<String> {
    if let Some(url) = args.backend_url.as_ref().filter(|s| !s.trim().is_empty()) {
        return Ok(url.trim_end_matches('/').to_string());
    }

    let host = args
        .host
        .clone()
        .or_else(|| std::env::var("HOST").ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let port = if let Some(p) = args.port {
        p
    } else if let Ok(port_str) = std::env::var("BACKEND_PORT").or_else(|_| std::env::var("PORT")) {
        port_str.parse::<u16>().context("invalid port value")?
    } else {
        read_port_file("vibe-kanban").await?
    };

    Ok(format!("http://{}:{}", host, port))
}

async fn load_info_task(base_url: String, net_tx: mpsc::Sender<NetEvent>) {
    let url = format!("{}/api/info", base_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(format!("reqwest init: {e}")))
                .await;
            return;
        }
    };

    let res = client.get(&url).send().await;
    match res {
        Ok(r) => {
            let parsed = r.json::<ApiResponse<serde_json::Value>>().await;
            match parsed {
                Ok(api) => {
                    let ok = api.is_success();
                    let summary = api
                        .into_data()
                        .and_then(|d| summarize_info(&d))
                        .unwrap_or_else(|| "loaded /api/info".to_string());
                    let _ = net_tx.send(NetEvent::InfoLoaded { ok, summary }).await;
                }
                Err(e) => {
                    let _ = net_tx
                        .send(NetEvent::InfoLoaded {
                            ok: false,
                            summary: format!("failed to parse /api/info: {e}"),
                        })
                        .await;
                }
            }
        }
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::InfoLoaded {
                    ok: false,
                    summary: format!("failed to fetch /api/info: {e}"),
                })
                .await;
        }
    }
}

fn summarize_info(info: &serde_json::Value) -> Option<String> {
    let env = info.get("environment")?;
    let os_type = env.get("os_type")?.as_str().unwrap_or("unknown");
    let os_arch = env
        .get("os_architecture")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let login_status = info.get("login_status")?;

    Some(format!(
        "env: {os_type} ({os_arch}) | login_status: {}",
        login_status_summary(login_status)
    ))
}

fn login_status_summary(v: &serde_json::Value) -> String {
    if v.get("LoggedOut").is_some() {
        return "logged_out".to_string();
    }
    if let Some(obj) = v.get("LoggedIn").and_then(|x| x.as_object()) {
        if let Some(user) = obj.get("user_id").and_then(|x| x.as_str()) {
            return format!("logged_in({})", &user[..user.len().min(8)]);
        }
        return "logged_in".to_string();
    }
    "unknown".to_string()
}

async fn projects_stream_task(
    base_url: String,
    mut reconnect_rx: watch::Receiver<u64>,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let mut backoff = Duration::from_millis(250);
    let max_backoff = Duration::from_secs(8);
    let endpoint = format!("{}/api/projects/stream/ws", base_url.trim_end_matches('/'));

    loop {
        let _ = net_tx
            .send(NetEvent::ProjectsStreamStatus(StreamStatus::Connecting))
            .await;

        match connect_ws(&endpoint).await {
            Ok(mut stream) => {
                let _ = net_tx
                    .send(NetEvent::ProjectsStreamStatus(StreamStatus::Connected))
                    .await;
                backoff = Duration::from_millis(250);

                loop {
                    tokio::select! {
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            break;
                        }
                        msg = stream.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(tungstenite::Message::Text(text)) => match parse_ws_message(&text) {
                                    WsParsed::Patch(patch) => {
                                        let _ = net_tx.send(NetEvent::ProjectsPatch(patch)).await;
                                    }
                                    WsParsed::Finished => {
                                        let _ = net_tx
                                            .send(NetEvent::ProjectsStreamStatus(
                                                StreamStatus::Disconnected,
                                            ))
                                            .await;
                                        return;
                                    }
                                    WsParsed::Ignored => {}
                                    WsParsed::Error(e) => {
                                        let _ = net_tx
                                            .send(NetEvent::Error(format!(
                                                "projects stream message error: {e}"
                                            )))
                                            .await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx
                                        .send(NetEvent::Error(format!("projects stream: {e}")))
                                        .await;
                                    break;
                                }
                            }
                        }
                    }
                }

                let _ = net_tx
                    .send(NetEvent::ProjectsStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::ProjectsStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetEvent::Error(format!("projects stream connect: {e}")))
                    .await;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            changed = reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
            }
        }
        backoff = (backoff * 2).min(max_backoff);
    }
}

async fn tasks_stream_task(
    base_url: String,
    mut project_rx: watch::Receiver<Option<Uuid>>,
    mut reconnect_rx: watch::Receiver<u64>,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let max_backoff = Duration::from_secs(8);
    let mut backoff = Duration::from_millis(250);

    loop {
        let project_id = *project_rx.borrow();
        let Some(project_id) = project_id else {
            let _ = net_tx.send(NetEvent::TasksReset).await;
            let _ = net_tx
                .send(NetEvent::TasksStreamStatus(StreamStatus::Disconnected))
                .await;
            tokio::select! {
                changed = project_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = reconnect_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
            continue;
        };

        let endpoint = format!(
            "{}/api/tasks/stream/ws?project_id={project_id}",
            base_url.trim_end_matches('/')
        );

        let _ = net_tx
            .send(NetEvent::TasksStreamStatus(StreamStatus::Connecting))
            .await;
        let _ = net_tx.send(NetEvent::TasksReset).await;

        match connect_ws(&endpoint).await {
            Ok(mut stream) => {
                let _ = net_tx
                    .send(NetEvent::TasksStreamStatus(StreamStatus::Connected))
                    .await;
                backoff = Duration::from_millis(250);

                loop {
                    tokio::select! {
                        changed = project_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::TasksReset).await;
                            break;
                        }
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::TasksReset).await;
                            break;
                        }
                        msg = stream.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(tungstenite::Message::Text(text)) => match parse_ws_message(&text) {
                                    WsParsed::Patch(patch) => {
                                        let _ = net_tx.send(NetEvent::TasksPatch(patch)).await;
                                    }
                                    WsParsed::Finished => {
                                        let _ = net_tx.send(NetEvent::TasksStreamStatus(StreamStatus::Disconnected)).await;
                                        return;
                                    }
                                    WsParsed::Ignored => {}
                                    WsParsed::Error(e) => {
                                        let _ = net_tx.send(NetEvent::Error(format!("tasks stream message error: {e}"))).await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx.send(NetEvent::Error(format!("tasks stream: {e}"))).await;
                                    break;
                                }
                            }
                        }
                    }
                }

                let _ = net_tx
                    .send(NetEvent::TasksStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::TasksStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetEvent::Error(format!("tasks stream connect: {e}")))
                    .await;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            changed = project_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
        }
        backoff = (backoff * 2).min(max_backoff);
    }
}

#[derive(Debug, serde::Deserialize)]
struct WorkspaceDto {
    id: Uuid,
    branch: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    setup_completed_at: Option<String>,
}

async fn load_attempts_task(base_url: String, task_id: Uuid, net_tx: mpsc::Sender<NetEvent>) {
    let url = format!(
        "{}/api/task-attempts?task_id={task_id}",
        base_url.trim_end_matches('/')
    );

    let client = match reqwest::Client::builder().build() {
        Ok(c) => c,
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(format!("reqwest init: {e}")))
                .await;
            return;
        }
    };

    match client.get(url).send().await {
        Ok(resp) => match resp.json::<ApiResponse<Vec<WorkspaceDto>>>().await {
            Ok(api) => {
                if !api.is_success() {
                    let _ = net_tx
                        .send(NetEvent::Error("failed to load task attempts".to_string()))
                        .await;
                    return;
                }
                let attempts = api
                    .into_data()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|w| AttemptRow {
                        id: w.id,
                        branch: w.branch,
                        created_at: w.created_at,
                        updated_at: w.updated_at,
                        setup_completed_at: w.setup_completed_at,
                    })
                    .collect();

                let _ = net_tx
                    .send(NetEvent::AttemptsLoaded { task_id, attempts })
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!(
                        "failed to parse task attempts response: {e}"
                    )))
                    .await;
            }
        },
        Err(e) => {
            let _ = net_tx
                .send(NetEvent::Error(format!(
                    "failed to fetch task attempts: {e}"
                )))
                .await;
        }
    }
}

async fn exec_stream_task(
    base_url: String,
    mut attempt_rx: watch::Receiver<Option<Uuid>>,
    mut reconnect_rx: watch::Receiver<u64>,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let max_backoff = Duration::from_secs(8);
    let mut backoff = Duration::from_millis(250);

    loop {
        let attempt_id = *attempt_rx.borrow();
        let Some(attempt_id) = attempt_id else {
            let _ = net_tx.send(NetEvent::ExecReset).await;
            let _ = net_tx
                .send(NetEvent::ExecStreamStatus(StreamStatus::Disconnected))
                .await;
            tokio::select! {
                changed = attempt_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = reconnect_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
            continue;
        };

        let endpoint = format!(
            "{}/api/execution-processes/stream/ws?workspace_id={attempt_id}",
            base_url.trim_end_matches('/')
        );

        let _ = net_tx
            .send(NetEvent::ExecStreamStatus(StreamStatus::Connecting))
            .await;
        let _ = net_tx.send(NetEvent::ExecReset).await;

        match connect_ws(&endpoint).await {
            Ok(mut stream) => {
                let _ = net_tx
                    .send(NetEvent::ExecStreamStatus(StreamStatus::Connected))
                    .await;
                backoff = Duration::from_millis(250);

                loop {
                    tokio::select! {
                        changed = attempt_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::ExecReset).await;
                            break;
                        }
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::ExecReset).await;
                            break;
                        }
                        msg = stream.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(tungstenite::Message::Text(text)) => match parse_ws_message(&text) {
                                    WsParsed::Patch(patch) => {
                                        let _ = net_tx.send(NetEvent::ExecPatch(patch)).await;
                                    }
                                    WsParsed::Finished => {
                                        let _ = net_tx.send(NetEvent::ExecStreamStatus(StreamStatus::Disconnected)).await;
                                        return;
                                    }
                                    WsParsed::Ignored => {}
                                    WsParsed::Error(e) => {
                                        let _ = net_tx.send(NetEvent::Error(format!("exec stream message error: {e}"))).await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx.send(NetEvent::Error(format!("exec stream: {e}"))).await;
                                    break;
                                }
                            }
                        }
                    }
                }

                let _ = net_tx
                    .send(NetEvent::ExecStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::ExecStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetEvent::Error(format!("exec stream connect: {e}")))
                    .await;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            changed = attempt_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
        }
        backoff = (backoff * 2).min(max_backoff);
    }
}

async fn diff_stream_task(
    base_url: String,
    mut attempt_rx: watch::Receiver<Option<Uuid>>,
    mut stats_only_rx: watch::Receiver<bool>,
    mut reconnect_rx: watch::Receiver<u64>,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let max_backoff = Duration::from_secs(8);
    let mut backoff = Duration::from_millis(250);

    loop {
        let attempt_id = *attempt_rx.borrow();
        let Some(attempt_id) = attempt_id else {
            let _ = net_tx.send(NetEvent::DiffReset).await;
            let _ = net_tx
                .send(NetEvent::DiffStreamStatus(StreamStatus::Disconnected))
                .await;
            tokio::select! {
                changed = attempt_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = stats_only_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = reconnect_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
            continue;
        };

        let stats_only = *stats_only_rx.borrow();
        let endpoint = format!(
            "{}/api/task-attempts/{attempt_id}/diff/ws?stats_only={stats_only}",
            base_url.trim_end_matches('/')
        );

        let _ = net_tx
            .send(NetEvent::DiffStreamStatus(StreamStatus::Connecting))
            .await;
        let _ = net_tx.send(NetEvent::DiffReset).await;

        match connect_ws(&endpoint).await {
            Ok(mut stream) => {
                let _ = net_tx
                    .send(NetEvent::DiffStreamStatus(StreamStatus::Connected))
                    .await;
                backoff = Duration::from_millis(250);

                loop {
                    tokio::select! {
                        changed = attempt_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::DiffReset).await;
                            break;
                        }
                        changed = stats_only_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::DiffReset).await;
                            break;
                        }
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::DiffReset).await;
                            break;
                        }
                        msg = stream.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(tungstenite::Message::Text(text)) => match parse_ws_message(&text) {
                                    WsParsed::Patch(patch) => {
                                        let _ = net_tx.send(NetEvent::DiffPatch(patch)).await;
                                    }
                                    WsParsed::Finished | WsParsed::Ignored => {}
                                    WsParsed::Error(e) => {
                                        let _ = net_tx.send(NetEvent::Error(format!("diff stream parse: {e}"))).await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx.send(NetEvent::Error(format!("diff stream: {e}"))).await;
                                    break;
                                }
                            }
                        }
                    }
                }

                let _ = net_tx
                    .send(NetEvent::DiffStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::DiffStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetEvent::Error(format!("diff stream connect: {e}")))
                    .await;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            changed = attempt_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = stats_only_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
        }
        backoff = (backoff * 2).min(max_backoff);
    }
}

async fn logs_stream_task(
    base_url: String,
    mut exec_rx: watch::Receiver<Option<Uuid>>,
    mut log_mode_rx: watch::Receiver<LogMode>,
    mut reconnect_rx: watch::Receiver<u64>,
    net_tx: mpsc::Sender<NetEvent>,
) {
    let max_backoff = Duration::from_secs(8);
    let mut backoff = Duration::from_millis(250);

    loop {
        let exec_id = *exec_rx.borrow();
        let Some(exec_id) = exec_id else {
            let _ = net_tx.send(NetEvent::LogReset).await;
            let _ = net_tx
                .send(NetEvent::LogStreamStatus(StreamStatus::Disconnected))
                .await;
            tokio::select! {
                changed = exec_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = log_mode_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = reconnect_rx.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
            continue;
        };

        let log_mode = *log_mode_rx.borrow();
        let endpoint = format!(
            "{}/api/execution-processes/{exec_id}/{}-logs/ws",
            base_url.trim_end_matches('/'),
            match log_mode {
                LogMode::Normalized => "normalized",
                LogMode::Raw => "raw",
            }
        );

        let _ = net_tx
            .send(NetEvent::LogStreamStatus(StreamStatus::Connecting))
            .await;
        let _ = net_tx.send(NetEvent::LogReset).await;

        match connect_ws(&endpoint).await {
            Ok(mut stream) => {
                let _ = net_tx
                    .send(NetEvent::LogStreamStatus(StreamStatus::Connected))
                    .await;
                backoff = Duration::from_millis(250);

                let mut finished = false;
                loop {
                    tokio::select! {
                        changed = exec_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::LogReset).await;
                            break;
                        }
                        changed = log_mode_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::LogReset).await;
                            break;
                        }
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                            let _ = net_tx.send(NetEvent::LogReset).await;
                            break;
                        }
                        msg = stream.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(tungstenite::Message::Text(text)) => match parse_ws_message(&text) {
                                    WsParsed::Patch(patch) => {
                                        let _ = net_tx.send(NetEvent::LogPatch(patch)).await;
                                    }
                                    WsParsed::Finished => {
                                        finished = true;
                                        break;
                                    }
                                    WsParsed::Ignored => {}
                                    WsParsed::Error(e) => {
                                        let _ = net_tx.send(NetEvent::Error(format!("log stream message error: {e}"))).await;
                                    }
                                },
                                Ok(tungstenite::Message::Close(_)) => break,
                                Ok(_) => {}
                                Err(e) => {
                                    let _ = net_tx.send(NetEvent::Error(format!("log stream: {e}"))).await;
                                    break;
                                }
                            }
                        }
                    }
                }

                if finished {
                    let _ = net_tx
                        .send(NetEvent::LogStreamStatus(StreamStatus::Completed))
                        .await;
                    tokio::select! {
                        changed = exec_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                        }
                        changed = log_mode_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                        }
                        changed = reconnect_rx.changed() => {
                            if changed.is_err() {
                                return;
                            }
                        }
                    }
                    continue;
                }

                let _ = net_tx
                    .send(NetEvent::LogStreamStatus(StreamStatus::Disconnected))
                    .await;

                continue;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::LogStreamStatus(StreamStatus::Error))
                    .await;
                let _ = net_tx
                    .send(NetEvent::Error(format!("log stream connect: {e}")))
                    .await;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            changed = exec_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = log_mode_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
            changed = reconnect_rx.changed() => {
                if changed.is_err() {
                    return;
                }
                continue;
            }
        }
        backoff = (backoff * 2).min(max_backoff);
    }
}

async fn connect_ws(
    http_url: &str,
) -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let ws_url = if let Some(rest) = http_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if let Some(rest) = http_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if http_url.starts_with("ws://") || http_url.starts_with("wss://") {
        http_url.to_string()
    } else {
        anyhow::bail!("unsupported URL scheme: {http_url}");
    };

    let (ws, _resp) = tokio_tungstenite::connect_async(ws_url).await?;
    Ok(ws)
}

enum WsParsed {
    Patch(json_patch::Patch),
    Finished,
    Ignored,
    Error(String),
}

fn parse_ws_message(text: &str) -> WsParsed {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return WsParsed::Error(e.to_string()),
    };

    if value.get("finished").and_then(|v| v.as_bool()) == Some(true) {
        return WsParsed::Finished;
    }

    let patch_value = match value.get("JsonPatch") {
        Some(v) => v.clone(),
        None => return WsParsed::Ignored,
    };

    let patch: json_patch::Patch = match serde_json::from_value(patch_value) {
        Ok(p) => p,
        Err(e) => return WsParsed::Error(e.to_string()),
    };

    WsParsed::Patch(patch)
}

fn reset_logs(app: &mut AppState) {
    app.log_store = serde_json::json!({ "entries": [] });
    app.log_lines.clear();
    app.log_line_entry_index.clear();
    app.log_entry_line_starts.clear();
    app.log_entry_end_states.clear();
    app.log_assembler_state = LogAssemblerState::default();
    app.pending_log_patch.0.clear();
    app.pending_log_dirty_from_entry = None;
    app.log_collapsed.clear();
    app.log_selected_entry = None;
    app.log_autoscroll = true;
    app.log_scroll_offset = 0;
}

fn enqueue_log_patch(app: &mut AppState, patch: json_patch::Patch) {
    if let Some(min_idx) = log_patch_min_entry_index(&patch) {
        app.pending_log_dirty_from_entry = Some(
            app.pending_log_dirty_from_entry
                .map(|v| v.min(min_idx))
                .unwrap_or(min_idx),
        );
    }
    app.pending_log_patch.0.extend(patch.0);
}

fn flush_log_patches(app: &mut AppState, width: usize) -> bool {
    let has_patch = !app.pending_log_patch.0.is_empty();
    let has_dirty = app.pending_log_dirty_from_entry.is_some();
    if !has_patch && !has_dirty {
        return false;
    }

    let width = width.max(1);
    let processed_entries_before = app.log_entry_line_starts.len();
    let rebuild_from = app
        .pending_log_dirty_from_entry
        .take()
        .unwrap_or(processed_entries_before);

    if has_patch {
        let patch = std::mem::take(&mut app.pending_log_patch);
        if let Err(e) = json_patch::patch(&mut app.log_store, &patch).context("apply patch") {
            app.last_error = Some(format!("failed to apply log patch: {e}"));
            app.log_status = StreamStatus::Error;
            return true;
        }
    } else {
        app.pending_log_patch.0.clear();
    }

    let entries = app
        .log_store
        .get("entries")
        .and_then(|v| v.as_array())
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    // Initialize collapse state for newly-seen entries.
    if app.log_collapsed.len() > entries.len() {
        app.log_collapsed.truncate(entries.len());
    }
    if app.log_collapsed.len() < entries.len() {
        let before = app.log_collapsed.len();
        app.log_collapsed.resize(entries.len(), false);
        for idx in before..entries.len() {
            app.log_collapsed[idx] = default_collapsed_for_log_entry(&entries[idx]);
        }
    }

    let rebuild_from = rebuild_from.min(processed_entries_before);
    if rebuild_from == 0 {
        app.log_lines.clear();
        app.log_line_entry_index.clear();
        app.log_entry_line_starts.clear();
        app.log_entry_end_states.clear();
        app.log_assembler_state = LogAssemblerState::default();
    } else if rebuild_from < processed_entries_before {
        let truncate_to = app.log_entry_line_starts[rebuild_from];
        app.log_lines.truncate(truncate_to);
        app.log_line_entry_index.truncate(truncate_to);
        app.log_entry_line_starts.truncate(rebuild_from);
        app.log_entry_end_states.truncate(rebuild_from);
        app.log_assembler_state = app.log_entry_end_states.last().copied().unwrap_or_default();
    }

    let log_mode = app.log_mode;
    let render_mode = app.log_render_mode;
    let diff_theme = app.diff_theme;

    let lines_before = app.log_lines.len();
    for idx in rebuild_from..entries.len() {
        let entry = &entries[idx];
        app.log_entry_line_starts.push(app.log_lines.len());
        append_log_entry(
            &mut app.log_lines,
            &mut app.log_line_entry_index,
            &mut app.log_assembler_state,
            idx,
            entry,
            width,
            log_mode,
            render_mode,
            diff_theme,
            &app.log_collapsed,
        );
        app.log_entry_end_states.push(app.log_assembler_state);
    }
    let lines_added = app.log_lines.len().saturating_sub(lines_before);

    if rebuild_from == processed_entries_before && lines_added > 0 && !app.log_autoscroll {
        app.log_scroll_offset = app.log_scroll_offset.saturating_add(lines_added);
    } else if app.log_autoscroll {
        app.log_scroll_offset = 0;
    }

    true
}

fn log_patch_min_entry_index(patch: &json_patch::Patch) -> Option<usize> {
    patch
        .iter()
        .filter_map(|op| {
            let path = op.path().to_string();
            let rest = path.strip_prefix("/entries/")?;
            let idx = rest.split('/').next()?;
            idx.parse::<usize>().ok()
        })
        .min()
}

fn toggle_selected_log_entry(app: &mut AppState) {
    let entries_len = app
        .log_store
        .get("entries")
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);
    if entries_len == 0 {
        return;
    }

    if app.log_collapsed.len() < entries_len {
        let before = app.log_collapsed.len();
        app.log_collapsed.resize(entries_len, false);
        for idx in before..entries_len {
            if let Some(entry) = app.log_store.get("entries").and_then(|v| v.get(idx)) {
                app.log_collapsed[idx] = default_collapsed_for_log_entry(entry);
            }
        }
    }

    let mut idx = app
        .log_selected_entry
        .unwrap_or(entries_len.saturating_sub(1));
    idx = idx.min(entries_len.saturating_sub(1));
    if let Some(v) = app.log_collapsed.get_mut(idx) {
        *v = !*v;
    }
    app.pending_log_dirty_from_entry = Some(
        app.pending_log_dirty_from_entry
            .map(|m| m.min(idx))
            .unwrap_or(idx),
    );
}

fn sanitize_tui_text(s: &str) -> std::borrow::Cow<'_, str> {
    // Control chars (especially '\r') and ANSI escape sequences can cause cursor movement and
    // visual corruption when written to the terminal. Strip them before rendering.
    fn needs_sanitize(s: &str) -> bool {
        s.as_bytes()
            .iter()
            .any(|&b| b == b'\x1b' || b == b'\r' || (b < 0x20 && b != b'\n') || b == 0x7f)
    }

    if !needs_sanitize(s) {
        return std::borrow::Cow::Borrowed(s);
    }

    #[derive(Clone, Copy, Debug)]
    enum State {
        Text,
        Esc,
        Csi,
        Osc,
    }

    let mut out = String::with_capacity(s.len());
    let mut state = State::Text;
    let mut osc_esc = false;

    for ch in s.chars() {
        match state {
            State::Text => match ch {
                '\x1b' => state = State::Esc,
                '\r' => {
                    // Drop CR to avoid carriage-return overwrites.
                }
                '\n' => {
                    // Preserve newlines for Markdown parsing and multi-line rendering.
                    out.push('\n');
                }
                '\t' => {
                    // Expand tabs to spaces for consistent width handling.
                    out.push_str("    ");
                }
                c if c.is_control() => {
                    // Drop other control chars; they can corrupt layout.
                }
                _ => out.push(ch),
            },
            State::Esc => {
                // ESC [ ... (CSI) or ESC ] ... (OSC); otherwise drop and return to text.
                match ch {
                    '[' => state = State::Csi,
                    ']' => {
                        state = State::Osc;
                        osc_esc = false;
                    }
                    _ => state = State::Text,
                }
            }
            State::Csi => {
                // Consume until final byte in the CSI range (@..~).
                if ('@'..='~').contains(&ch) {
                    state = State::Text;
                }
            }
            State::Osc => {
                // Consume OSC until BEL or ST (ESC \).
                if osc_esc {
                    if ch == '\\' {
                        state = State::Text;
                    }
                    osc_esc = false;
                } else if ch == '\x07' {
                    state = State::Text;
                } else if ch == '\x1b' {
                    osc_esc = true;
                }
            }
        }
    }

    std::borrow::Cow::Owned(out)
}

fn style_for_log_kind(kind: LogKind) -> Style {
    match kind {
        LogKind::Stdout => Style::default(),
        LogKind::Stderr => Style::default().fg(Color::Red),
        LogKind::Info => Style::default().fg(Color::Cyan),
    }
}

fn default_collapsed_for_log_entry(entry: &serde_json::Value) -> bool {
    const THRESHOLD_LINES: usize = 24;

    let Some(ty) = entry.get("type").and_then(|v| v.as_str()) else {
        return false;
    };
    if ty != "NORMALIZED_ENTRY" {
        return false;
    }

    let Some(content) = entry.get("content") else {
        return false;
    };
    let Some(entry_type) = content.get("entry_type") else {
        return false;
    };
    let Some(entry_type_tag) = entry_type.get("type").and_then(|v| v.as_str()) else {
        return false;
    };
    if entry_type_tag != "tool_use" {
        return false;
    }

    let Some(action_type) = entry_type.get("action_type") else {
        return false;
    };
    let Some(action) = action_type.get("action").and_then(|v| v.as_str()) else {
        return false;
    };

    match action {
        "command_run" => {
            let output = action_type
                .get("result")
                .and_then(|v| v.get("output"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            output.lines().count() > THRESHOLD_LINES
        }
        "file_edit" => {
            let mut lines = 0usize;
            let changes = action_type.get("changes").and_then(|v| v.as_array());
            for c in changes.into_iter().flatten() {
                if c.get("action").and_then(|v| v.as_str()) == Some("edit") {
                    let diff = c.get("unified_diff").and_then(|v| v.as_str()).unwrap_or("");
                    lines = lines.saturating_add(diff.lines().count());
                    if lines > THRESHOLD_LINES {
                        return true;
                    }
                }
            }
            false
        }
        "tool" => {
            let result_type = action_type
                .get("result")
                .and_then(|v| v.get("type"))
                .and_then(|v| v.as_str());
            let value = action_type.get("result").and_then(|v| v.get("value"));
            match (result_type, value) {
                (Some("markdown"), Some(v)) => v
                    .as_str()
                    .unwrap_or("")
                    .lines()
                    .count()
                    .gt(&THRESHOLD_LINES),
                (Some("json"), Some(v)) => serde_json::to_string_pretty(v)
                    .ok()
                    .map(|s| s.lines().count() > THRESHOLD_LINES)
                    .unwrap_or(false),
                _ => false,
            }
        }
        _ => false,
    }
}

fn tool_status_str(entry_type: &serde_json::Value) -> Option<&str> {
    let status = entry_type.get("status")?;
    if let Some(s) = status.as_str() {
        return Some(s);
    }
    status.get("status").and_then(|v| v.as_str())
}

fn tool_status_badge(status: Option<&str>) -> (Span<'static>, Style) {
    match status.unwrap_or("created") {
        "success" => (
            Span::styled("ok", Style::default().fg(Color::Green)),
            Style::default().fg(Color::Green),
        ),
        "failed" => (
            Span::styled("fail", Style::default().fg(Color::Red)),
            Style::default().fg(Color::Red),
        ),
        "denied" => (
            Span::styled("denied", Style::default().fg(Color::Yellow)),
            Style::default().fg(Color::Yellow),
        ),
        "pending_approval" => (
            Span::styled("approval", Style::default().fg(Color::Magenta)),
            Style::default().fg(Color::Magenta),
        ),
        "timed_out" => (
            Span::styled("timeout", Style::default().fg(Color::Yellow)),
            Style::default().fg(Color::Yellow),
        ),
        _ => (
            Span::styled("…", Style::default().add_modifier(Modifier::DIM)),
            Style::default().add_modifier(Modifier::DIM),
        ),
    }
}

fn line_display_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|s| display_width(s.content.as_ref()))
        .sum()
}

fn split_token_prefer_separators(s: &str, max: usize) -> (String, String) {
    if max == 0 {
        return (String::new(), s.to_string());
    }
    if display_width(s) <= max {
        return (s.to_string(), String::new());
    }

    let mut chunk = String::new();
    let mut last_soft_break: Option<usize> = None;
    for (i, ch) in s.char_indices() {
        let next = format!("{chunk}{ch}");
        if display_width(&next) > max {
            break;
        }
        chunk.push(ch);
        let end = i + ch.len_utf8();
        if matches!(
            ch,
            '/' | '-' | '_' | '.' | ':' | '@' | '?' | '&' | '=' | '#'
        ) {
            last_soft_break = Some(end);
        }
    }

    let cut = last_soft_break.unwrap_or_else(|| {
        let (c, _) = split_by_width(s, max);
        c.len()
    });

    let mut left = s.get(..cut).unwrap_or("").to_string();
    let mut right = s.get(cut..).unwrap_or("").to_string();
    // Trim spaces around the break when breaking at a separator boundary.
    left = left.trim_end().to_string();
    right = right.trim_start().to_string();
    (left, right)
}

fn wrap_line_wordwise(line: &Line<'static>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    if line.spans.is_empty() || line_display_width(line) <= width {
        return vec![line.clone()];
    }

    #[derive(Clone)]
    struct Tok {
        text: String,
        style: Style,
        is_ws: bool,
    }

    let mut tokens: Vec<Tok> = vec![];
    for span in &line.spans {
        let style = span.style;
        let text = span.content.as_ref();
        if text.is_empty() {
            continue;
        }
        let mut cur = String::new();
        let mut cur_ws: Option<bool> = None;
        for ch in text.chars() {
            let is_ws = ch.is_whitespace();
            if cur_ws == Some(is_ws) || cur_ws.is_none() {
                cur.push(ch);
                cur_ws = Some(is_ws);
            } else {
                tokens.push(Tok {
                    text: cur.clone(),
                    style,
                    is_ws: cur_ws.unwrap_or(false),
                });
                cur.clear();
                cur.push(ch);
                cur_ws = Some(is_ws);
            }
        }
        if !cur.is_empty() {
            tokens.push(Tok {
                text: cur,
                style,
                is_ws: cur_ws.unwrap_or(false),
            });
        }
    }

    let mut out: Vec<Line<'static>> = vec![];
    let mut cur_spans: Vec<Span<'static>> = vec![];
    let mut cur_w: usize = 0;

    let mut i = 0usize;
    while i < tokens.len() {
        let tok = tokens[i].clone();
        if tok.is_ws {
            // Avoid starting wrapped lines with incidental whitespace.
            if cur_spans.is_empty() {
                // Keep indentation (2+ spaces) if the original line started with it.
                if tok.text.chars().all(|c| c == ' ') && tok.text.len() >= 2 {
                    let w = display_width(&tok.text);
                    if w <= width {
                        push_span_merged(&mut cur_spans, tok.text, tok.style);
                        cur_w += w;
                    }
                }
                i += 1;
                continue;
            }

            // Normalize whitespace between words to a single space for wrapping.
            let space = " ".to_string();
            let w = 1usize;
            if cur_w + w > width {
                out.push(Line::from(cur_spans.clone()));
                cur_spans.clear();
                cur_w = 0;
                i += 1;
                continue;
            }
            push_span_merged(&mut cur_spans, space, tok.style);
            cur_w += w;
            i += 1;
            continue;
        }

        // Non-whitespace token
        let mut text = tok.text;
        let style = tok.style;
        loop {
            let w = display_width(&text);
            if cur_w + w <= width {
                push_span_merged(&mut cur_spans, text, style);
                cur_w += w;
                break;
            }

            if !cur_spans.is_empty() {
                // Word-wrap: move token to next line.
                out.push(Line::from(cur_spans.clone()));
                cur_spans.clear();
                cur_w = 0;
                continue;
            }

            // Token longer than the full line: split on soft separators first.
            let (chunk, rest) = split_token_prefer_separators(&text, width);
            if chunk.is_empty() {
                let (c, r) = split_by_width(&text, width);
                if c.is_empty() {
                    break;
                }
                push_span_merged(&mut cur_spans, c, style);
                out.push(Line::from(cur_spans.clone()));
                cur_spans.clear();
                cur_w = 0;
                text = r;
                if text.is_empty() {
                    break;
                }
                continue;
            }
            let chunk_w = display_width(&chunk);
            push_span_merged(&mut cur_spans, chunk, style);
            cur_w += chunk_w;
            out.push(Line::from(cur_spans.clone()));
            cur_spans.clear();
            cur_w = 0;
            text = rest;
            if text.is_empty() {
                break;
            }
        }
        i += 1;
    }

    if !cur_spans.is_empty() {
        out.push(Line::from(cur_spans));
    }

    out
}

fn push_line(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    entry_idx: usize,
    line: Line<'static>,
    width: usize,
) {
    for wrapped in wrap_line_wordwise(&line, width) {
        lines.push(wrapped);
        map.push(entry_idx);
    }
}

fn append_log_entry(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    state: &mut LogAssemblerState,
    entry_idx: usize,
    entry: &serde_json::Value,
    width: usize,
    log_mode: LogMode,
    render_mode: LogRenderMode,
    diff_theme: DiffTheme,
    collapsed: &[bool],
) {
    fn line_is_blank(line: &Line<'static>) -> bool {
        line.spans
            .iter()
            .all(|s| s.content.as_ref().trim().is_empty())
    }

    let Some(ty) = entry.get("type").and_then(|v| v.as_str()) else {
        return;
    };

    match ty {
        "STDOUT" => {
            let Some(text) = entry.get("content").and_then(|v| v.as_str()) else {
                return;
            };
            let target_idx = state.attach_to_entry.unwrap_or(entry_idx);
            if collapsed.get(target_idx).copied().unwrap_or(false) {
                return;
            }
            append_stream_text(
                lines,
                map,
                state,
                LogKind::Stdout,
                text,
                Style::default(),
                true,
                target_idx,
                "  ",
                width,
            );
        }
        "STDERR" => {
            let Some(text) = entry.get("content").and_then(|v| v.as_str()) else {
                return;
            };
            let target_idx = state.attach_to_entry.unwrap_or(entry_idx);
            if collapsed.get(target_idx).copied().unwrap_or(false) {
                return;
            }
            append_stream_text(
                lines,
                map,
                state,
                LogKind::Stderr,
                text,
                Style::default().fg(Color::Red),
                true,
                target_idx,
                "  ",
                width,
            );
        }
        "NORMALIZED_ENTRY" => {
            let Some(content) = entry.get("content") else {
                return;
            };

            // Don't join stdout/stderr across normalized entries.
            state.open = false;
            state.open_kind = None;
            state.attach_to_entry = None;

            if log_mode == LogMode::Raw {
                // Raw mode intentionally focuses on stdout/stderr.
                return;
            }

            let entry_type_tag = content
                .get("entry_type")
                .and_then(|v| v.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let is_progress = matches!(entry_type_tag, "thinking" | "loading");

            // Visual separation between "cards"/blocks, but don't spam blank lines for
            // ephemeral progress entries (thinking/loading).
            if is_progress {
                // If we're starting a new progress sequence (i.e. previous block wasn't a progress
                // update), add the same separation we use for other blocks.
                if state.progress_kind.is_none() {
                    if let Some(last) = lines.last() {
                        if !line_is_blank(last) {
                            let sep_owner = map.last().copied().unwrap_or(entry_idx);
                            push_line(lines, map, sep_owner, Line::from(""), width);
                        }
                    }
                }
            } else {
                // When a "real" entry arrives, stop coalescing progress.
                state.progress_kind = None;
                state.progress_count = 0;
                state.progress_line_pos = None;

                if let Some(last) = lines.last() {
                    if !line_is_blank(last) {
                        let sep_owner = map.last().copied().unwrap_or(entry_idx);
                        push_line(lines, map, sep_owner, Line::from(""), width);
                    }
                }
            }

            append_normalized_entry(
                lines,
                map,
                state,
                entry_idx,
                content,
                width,
                render_mode,
                diff_theme,
                collapsed.get(entry_idx).copied().unwrap_or(false),
            );
        }
        _ => {}
    }
}

fn append_stream_text(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    state: &mut LogAssemblerState,
    kind: LogKind,
    text: &str,
    style: Style,
    allow_join: bool,
    entry_idx: usize,
    prefix: &str,
    width: usize,
) {
    if text.is_empty() {
        return;
    }

    let ends_with_newline = text.ends_with('\n');
    let mut is_first = true;
    for raw in text.split_terminator('\n') {
        let seg = raw.strip_suffix('\r').unwrap_or(raw);
        let seg = sanitize_tui_text(seg);

        if allow_join
            && is_first
            && state.open
            && state.open_kind == Some(kind)
            && map.last().copied() == Some(entry_idx)
            && lines
                .last()
                .is_some_and(|l| l.spans.len() == 1 && l.spans[0].style == style)
        {
            if let Some(last) = lines.last_mut() {
                if let Some(span) = last.spans.first_mut() {
                    span.content.to_mut().push_str(seg.as_ref());
                }
            }
            // If joining caused the line to overflow, re-wrap it.
            if lines
                .last()
                .is_some_and(|l| line_display_width(l) > width.max(1))
            {
                let line = lines.pop().unwrap();
                let _ = map.pop();
                push_line(lines, map, entry_idx, line, width);
            }
        } else {
            push_line(
                lines,
                map,
                entry_idx,
                Line::from(Span::styled(format!("{prefix}{}", seg), style)),
                width,
            );
        }

        is_first = false;
    }

    if allow_join && !ends_with_newline {
        state.open = true;
        state.open_kind = Some(kind);
    } else {
        state.open = false;
        state.open_kind = None;
    }
}

fn append_normalized_entry(
    lines: &mut Vec<Line<'static>>,
    map: &mut Vec<usize>,
    state: &mut LogAssemblerState,
    entry_idx: usize,
    entry: &serde_json::Value,
    width: usize,
    render_mode: LogRenderMode,
    diff_theme: DiffTheme,
    collapsed: bool,
) {
    fn progress_line(kind: ProgressKind, count: usize, width: usize) -> Line<'static> {
        let label = match kind {
            ProgressKind::Thinking => "thinking…",
            ProgressKind::Loading => "loading…",
        };
        let text = if count > 1 {
            format!("{label} (x{count})")
        } else {
            label.to_string()
        };

        // Keep this to a single line to make in-place updates easy.
        let max = width.saturating_sub(2).max(1);
        let text = truncate_to_width(&text, max);
        Line::from(vec![
            Span::styled("▌", Style::default().add_modifier(Modifier::DIM)),
            Span::raw(" "),
            Span::styled(text, Style::default().add_modifier(Modifier::DIM)),
        ])
    }

    fn append_text_block(
        lines: &mut Vec<Line<'static>>,
        map: &mut Vec<usize>,
        entry_idx: usize,
        label: &str,
        accent: Color,
        text: &str,
        width: usize,
        render_mode: LogRenderMode,
    ) {
        let header = Line::from(vec![
            Span::styled("▌", Style::default().fg(accent)),
            Span::raw(" "),
            Span::styled(
                label.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]);
        push_line(lines, map, entry_idx, header, width);

        if text.trim().is_empty() {
            return;
        }

        let body_lines = if render_mode == LogRenderMode::Markdown {
            render_markdown(
                text,
                width.saturating_sub(2).max(1),
                MdSoftBreakMode::Newline,
            )
        } else {
            text.lines()
                .map(|l| Line::from(Span::raw(l.to_string())))
                .collect()
        };
        for l in body_lines {
            let mut spans = vec![Span::styled(
                "  ",
                Style::default().add_modifier(Modifier::DIM),
            )];
            spans.extend(l.spans.into_iter());
            push_line(lines, map, entry_idx, Line::from(spans), width);
        }
    }

    let entry_type = entry.get("entry_type");
    let entry_type = match entry_type {
        Some(v) => v,
        None => {
            let fallback = entry.to_string();
            push_line(lines, map, entry_idx, Line::from(fallback), width);
            return;
        }
    };

    let entry_type_tag = entry_type
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let content_text = entry.get("content").and_then(|v| v.as_str()).unwrap_or("");

    match entry_type_tag {
        "user_message" => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "You",
                Color::Yellow,
                content_text,
                width,
                render_mode,
            );
        }
        "assistant_message" => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "Assistant",
                Color::Cyan,
                content_text,
                width,
                render_mode,
            );
        }
        "system_message" => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "System",
                Color::Gray,
                content_text,
                width,
                render_mode,
            );
        }
        "error_message" => {
            append_text_block(
                lines,
                map,
                entry_idx,
                "Error",
                Color::Red,
                content_text,
                width,
                render_mode,
            );
        }
        "user_feedback" => {
            let denied_tool = entry_type
                .get("denied_tool")
                .and_then(|v| v.as_str())
                .unwrap_or("tool");
            append_text_block(
                lines,
                map,
                entry_idx,
                &format!("Feedback (denied {denied_tool})"),
                Color::Yellow,
                content_text,
                width,
                render_mode,
            );
        }
        "thinking" => {
            let kind = ProgressKind::Thinking;
            if state.progress_kind == Some(kind)
                && let Some(pos) = state.progress_line_pos
                && pos < lines.len()
            {
                state.progress_count = state.progress_count.saturating_add(1);
                lines[pos] = progress_line(kind, state.progress_count, width);
                map[pos] = entry_idx;
            } else {
                state.progress_kind = Some(kind);
                state.progress_count = 1;
                state.progress_line_pos = Some(lines.len());
                push_line(lines, map, entry_idx, progress_line(kind, 1, width), width);
            }
        }
        "loading" => {
            let kind = ProgressKind::Loading;
            if state.progress_kind == Some(kind)
                && let Some(pos) = state.progress_line_pos
                && pos < lines.len()
            {
                state.progress_count = state.progress_count.saturating_add(1);
                lines[pos] = progress_line(kind, state.progress_count, width);
                map[pos] = entry_idx;
            } else {
                state.progress_kind = Some(kind);
                state.progress_count = 1;
                state.progress_line_pos = Some(lines.len());
                push_line(lines, map, entry_idx, progress_line(kind, 1, width), width);
            }
        }
        "next_action" => {
            let failed = entry_type
                .get("failed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let needs_setup = entry_type
                .get("needs_setup")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let procs = entry_type
                .get("execution_processes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let text = format!(
                "next action{} (execs: {}, setup: {})",
                if failed { " (failed)" } else { "" },
                procs,
                if needs_setup { "needed" } else { "ok" }
            );
            push_line(
                lines,
                map,
                entry_idx,
                Line::from(Span::styled(
                    text,
                    Style::default().add_modifier(Modifier::DIM),
                )),
                width,
            );
        }
        "tool_use" => {
            let status = tool_status_str(entry_type);
            let (status_badge, _status_style) = tool_status_badge(status);

            let action_type = entry_type.get("action_type");
            let action_type = match action_type {
                Some(v) => v,
                None => {
                    append_text_block(
                        lines,
                        map,
                        entry_idx,
                        "Tool",
                        Color::Blue,
                        content_text,
                        width,
                        render_mode,
                    );
                    return;
                }
            };
            let action = action_type
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("other");

            let (label, accent) = match action {
                "file_read" | "search" => ("Explored", Color::Cyan),
                "file_edit" => ("Edited", Color::Green),
                "command_run" => ("Ran", Color::Cyan),
                "web_fetch" => ("Fetched", Color::Cyan),
                "task_create" => ("Created", Color::Green),
                "plan_presentation" => ("Plan", Color::Magenta),
                "todo_management" => ("Todos", Color::Magenta),
                "tool" => ("Tool", Color::Blue),
                _ => ("Tool", Color::Blue),
            };

            let arrow = if collapsed { "▸" } else { "▾" };

            let mut header_spans: Vec<Span<'static>> = vec![
                Span::styled("▌", Style::default().fg(accent)),
                Span::raw(" "),
                Span::styled(
                    arrow.to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::raw(" "),
            ];

            // Put the most important detail in the header for scannability.
            match action {
                "command_run" => {
                    let cmd = action_type
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("command");
                    header_spans.push(Span::styled(
                        format!("{label} {cmd}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                }
                "file_edit" => {
                    let path = action_type
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("file");
                    header_spans.push(Span::styled(
                        format!("{label} {path}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                }
                _ => {
                    header_spans.push(Span::styled(
                        label.to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                }
            }

            header_spans.push(Span::raw(" ("));
            header_spans.push(status_badge);
            header_spans.push(Span::raw(")"));
            push_line(lines, map, entry_idx, Line::from(header_spans), width);

            match action {
                "file_read" => {
                    let path = action_type
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("file");
                    push_line(
                        lines,
                        map,
                        entry_idx,
                        Line::from(vec![
                            Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                            Span::raw(format!("Read {path}")),
                        ]),
                        width,
                    );
                }
                "search" => {
                    let query = action_type
                        .get("query")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    push_line(
                        lines,
                        map,
                        entry_idx,
                        Line::from(vec![
                            Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                            Span::raw(format!("Search {query}")),
                        ]),
                        width,
                    );
                }
                "command_run" => {
                    state.attach_to_entry = Some(entry_idx);

                    let exit_status = action_type.get("result").and_then(|v| v.get("exit_status"));
                    if let Some(es) = exit_status {
                        let code = es
                            .get("code")
                            .and_then(|v| v.as_i64())
                            .or_else(|| es.as_i64())
                            .or_else(|| es.as_u64().map(|v| v as i64));
                        if let Some(code) = code
                            && code != 0
                        {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  - ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        format!("exit {code}"),
                                        Style::default().fg(Color::Red),
                                    ),
                                ]),
                                width,
                            );
                        }
                    }

                    let output = action_type
                        .get("result")
                        .and_then(|v| v.get("output"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !output.is_empty() {
                        if collapsed {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  ▸ ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        "output (collapsed)".to_string(),
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                ]),
                                width,
                            );
                        } else {
                            const MAX_OUTPUT_LINES: usize = 400;
                            for (i, l) in output.lines().take(MAX_OUTPUT_LINES).enumerate() {
                                let line = sanitize_tui_text(l.strip_suffix('\r').unwrap_or(l));
                                push_line(
                                    lines,
                                    map,
                                    entry_idx,
                                    Line::from(Span::styled(
                                        format!("  {}", line),
                                        Style::default().add_modifier(Modifier::DIM),
                                    )),
                                    width,
                                );
                                if i + 1 == MAX_OUTPUT_LINES {
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(Span::styled(
                                            "  … (truncated)".to_string(),
                                            Style::default().add_modifier(Modifier::DIM),
                                        )),
                                        width,
                                    );
                                }
                            }
                        }
                    }
                }
                "web_fetch" => {
                    let url = action_type
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    push_line(
                        lines,
                        map,
                        entry_idx,
                        Line::from(vec![
                            Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                            Span::raw(format!("GET {url}")),
                        ]),
                        width,
                    );
                }
                "file_edit" => {
                    let path = action_type
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("file");
                    let changes = action_type
                        .get("changes")
                        .and_then(|v| v.as_array())
                        .cloned();
                    if let Some(changes) = changes {
                        let mut has_diff = false;
                        for c in &changes {
                            let action =
                                c.get("action").and_then(|v| v.as_str()).unwrap_or("change");
                            match action {
                                "write" => {
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(vec![
                                            Span::styled(
                                                "  - ",
                                                Style::default().add_modifier(Modifier::DIM),
                                            ),
                                            Span::raw("Write content".to_string()),
                                        ]),
                                        width,
                                    );
                                }
                                "delete" => {
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(vec![
                                            Span::styled(
                                                "  - ",
                                                Style::default().add_modifier(Modifier::DIM),
                                            ),
                                            Span::raw("Delete file".to_string()),
                                        ]),
                                        width,
                                    );
                                }
                                "rename" => {
                                    let new_path = c
                                        .get("new_path")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("new path");
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(vec![
                                            Span::styled(
                                                "  - ",
                                                Style::default().add_modifier(Modifier::DIM),
                                            ),
                                            Span::raw(format!("Rename → {new_path}")),
                                        ]),
                                        width,
                                    );
                                }
                                "edit" => {
                                    has_diff = true;
                                }
                                _ => {}
                            }
                        }

                        if has_diff {
                            if collapsed {
                                push_line(
                                    lines,
                                    map,
                                    entry_idx,
                                    Line::from(vec![
                                        Span::styled(
                                            "  ▸ ",
                                            Style::default().add_modifier(Modifier::DIM),
                                        ),
                                        Span::styled(
                                            "diff (collapsed)".to_string(),
                                            Style::default().add_modifier(Modifier::DIM),
                                        ),
                                    ]),
                                    width,
                                );
                            } else {
                                const MAX_DIFF_LINES: usize = 300;
                                for c in &changes {
                                    if c.get("action").and_then(|v| v.as_str()) != Some("edit") {
                                        continue;
                                    }
                                    let diff = c
                                        .get("unified_diff")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    if diff.trim().is_empty() {
                                        continue;
                                    }
                                    let body_width = width.saturating_sub(2).max(1);
                                    let mut rendered = highlight_unified_diff(
                                        path, diff, body_width, diff_theme, false,
                                    );
                                    if rendered.len() > MAX_DIFF_LINES {
                                        rendered.truncate(MAX_DIFF_LINES);
                                        rendered.push(Line::from(Span::styled(
                                            "… (truncated)".to_string(),
                                            Style::default().add_modifier(Modifier::DIM),
                                        )));
                                    }
                                    for l in rendered {
                                        let mut spans: Vec<Span<'static>> = vec![Span::styled(
                                            "  ",
                                            Style::default().add_modifier(Modifier::DIM),
                                        )];
                                        spans.extend(l.spans.into_iter());
                                        push_line(lines, map, entry_idx, Line::from(spans), width);
                                    }
                                }
                            }
                        }
                    } else {
                        push_line(
                            lines,
                            map,
                            entry_idx,
                            Line::from(vec![
                                Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                                Span::raw(format!("Edit {path}")),
                            ]),
                            width,
                        );
                    }
                }
                "task_create" => {
                    let description = action_type
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !description.trim().is_empty() {
                        if collapsed {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  ▸ ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        "details (collapsed)".to_string(),
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                ]),
                                width,
                            );
                        } else {
                            for l in render_markdown(
                                description,
                                width.saturating_sub(2).max(1),
                                MdSoftBreakMode::Space,
                            ) {
                                let mut spans = vec![Span::styled(
                                    "  ",
                                    Style::default().add_modifier(Modifier::DIM),
                                )];
                                spans.extend(l.spans.into_iter());
                                push_line(lines, map, entry_idx, Line::from(spans), width);
                            }
                        }
                    }
                }
                "plan_presentation" => {
                    let plan = action_type
                        .get("plan")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if !plan.trim().is_empty() {
                        if collapsed {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  ▸ ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        "plan (collapsed)".to_string(),
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                ]),
                                width,
                            );
                        } else {
                            for l in render_markdown(
                                plan,
                                width.saturating_sub(2).max(1),
                                MdSoftBreakMode::Space,
                            ) {
                                let mut spans = vec![Span::styled(
                                    "  ",
                                    Style::default().add_modifier(Modifier::DIM),
                                )];
                                spans.extend(l.spans.into_iter());
                                push_line(lines, map, entry_idx, Line::from(spans), width);
                            }
                        }
                    }
                }
                "todo_management" => {
                    let todos = action_type.get("todos").and_then(|v| v.as_array()).cloned();
                    if let Some(todos) = todos {
                        let count = todos.len();
                        push_line(
                            lines,
                            map,
                            entry_idx,
                            Line::from(vec![
                                Span::styled("  - ", Style::default().add_modifier(Modifier::DIM)),
                                Span::raw(format!("{count} todos")),
                            ]),
                            width,
                        );
                        if !collapsed {
                            for t in todos.iter().take(50) {
                                let content =
                                    t.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                let status = t.get("status").and_then(|v| v.as_str()).unwrap_or("");
                                let mark = if status == "done" { "[x]" } else { "[ ]" };
                                push_line(
                                    lines,
                                    map,
                                    entry_idx,
                                    Line::from(vec![
                                        Span::styled(
                                            "  ",
                                            Style::default().add_modifier(Modifier::DIM),
                                        ),
                                        Span::raw(format!("{mark} {content}")),
                                    ]),
                                    width,
                                );
                            }
                        }
                    }
                }
                "tool" => {
                    let tool_name = action_type
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool");
                    let args = action_type.get("arguments");
                    if let Some(args) = args {
                        if let Ok(pretty) = serde_json::to_string_pretty(args) {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  - ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::raw(format!("{tool_name} args")),
                                ]),
                                width,
                            );
                            if !collapsed {
                                for l in pretty.lines().take(80) {
                                    let l = sanitize_tui_text(l);
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(Span::styled(
                                            format!("  {}", l),
                                            Style::default().add_modifier(Modifier::DIM),
                                        )),
                                        width,
                                    );
                                }
                            }
                        }
                    }

                    let result_ty = action_type
                        .get("result")
                        .and_then(|v| v.get("type"))
                        .and_then(|v| v.as_str());
                    let value = action_type.get("result").and_then(|v| v.get("value"));
                    if let (Some(result_ty), Some(value)) = (result_ty, value) {
                        if collapsed {
                            push_line(
                                lines,
                                map,
                                entry_idx,
                                Line::from(vec![
                                    Span::styled(
                                        "  ▸ ",
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                    Span::styled(
                                        "result (collapsed)".to_string(),
                                        Style::default().add_modifier(Modifier::DIM),
                                    ),
                                ]),
                                width,
                            );
                        } else if result_ty == "markdown" {
                            let md = value.as_str().unwrap_or("");
                            for l in render_markdown(
                                md,
                                width.saturating_sub(2).max(1),
                                MdSoftBreakMode::Space,
                            ) {
                                let mut spans = vec![Span::styled(
                                    "  ",
                                    Style::default().add_modifier(Modifier::DIM),
                                )];
                                spans.extend(l.spans.into_iter());
                                push_line(lines, map, entry_idx, Line::from(spans), width);
                            }
                        } else if result_ty == "json" {
                            if let Ok(pretty) = serde_json::to_string_pretty(value) {
                                for l in pretty.lines().take(200) {
                                    let l = sanitize_tui_text(l);
                                    push_line(
                                        lines,
                                        map,
                                        entry_idx,
                                        Line::from(Span::styled(
                                            format!("  {}", l),
                                            Style::default().add_modifier(Modifier::DIM),
                                        )),
                                        width,
                                    );
                                }
                            }
                        }
                    }
                }
                _ => {
                    if !content_text.trim().is_empty() {
                        append_text_block(
                            lines,
                            map,
                            entry_idx,
                            label,
                            accent,
                            content_text,
                            width,
                            render_mode,
                        );
                    }
                }
            }
        }
        other => {
            // Fallback: preserve existing behavior.
            let fallback = other.replace('_', " ");
            if !content_text.trim().is_empty() {
                append_text_block(
                    lines,
                    map,
                    entry_idx,
                    &fallback,
                    Color::Gray,
                    content_text,
                    width,
                    render_mode,
                );
            } else if let Some(text) = normalized_entry_text(entry) {
                let mut rendered = if render_mode == LogRenderMode::Markdown {
                    render_markdown(&text, width.max(1), MdSoftBreakMode::Newline)
                } else {
                    text.lines()
                        .map(|l| Line::from(Span::raw(l.to_string())))
                        .collect()
                };
                if rendered.is_empty() {
                    rendered.push(Line::from(""));
                }
                for l in rendered {
                    push_line(lines, map, entry_idx, l, width);
                }
            }
        }
    }
}

fn normalized_entry_text(entry: &serde_json::Value) -> Option<String> {
    let content = entry
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !content.trim().is_empty() {
        return Some(content);
    }

    let entry_type = entry.get("entry_type")?;
    let ty = entry_type.get("type").and_then(|v| v.as_str())?;
    let label = match ty {
        "tool_use" => {
            let tool = entry_type
                .get("tool_name")
                .and_then(|v| v.as_str())
                .unwrap_or("tool");
            let status = entry_type
                .get("status")
                .and_then(|v| v.get("status"))
                .and_then(|v| v.as_str())
                .unwrap_or("created");
            format!("{tool} ({status})")
        }
        "next_action" => "next action".to_string(),
        "loading" => "loading…".to_string(),
        "thinking" => "thinking…".to_string(),
        other => other.replace('_', " "),
    };

    Some(label)
}

#[derive(Debug, Clone)]
enum MdToken {
    Text(String, Style),
    Newline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MdSoftBreakMode {
    Space,
    Newline,
}

fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn truncate_to_width(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if display_width(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars() {
        if display_width(&out) >= max.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

fn hash_text_sample(hasher: &mut impl Hasher, s: &str) {
    s.len().hash(hasher);
    let bytes = s.as_bytes();
    let take = bytes.len().min(4096);
    bytes[..take].hash(hasher);
    if bytes.len() > take {
        bytes[bytes.len().saturating_sub(take)..].hash(hasher);
    }
}

fn refresh_diff_preview_cache(app: &mut AppState, width: usize) -> bool {
    let width_u16 = (width.min(u16::MAX as usize)) as u16;
    if app.diff_preview_cache_width != width_u16 {
        app.diff_preview_cache_width = width_u16;
        app.diff_preview_cache_key = None;
    }

    let rows = diff_rows_with_all(&app.diff_store);
    let selected = rows
        .get(app.selected_diff_index.min(rows.len().saturating_sub(1)))
        .cloned();

    let Some(selected) = selected else {
        if app.diff_preview_cache_key.is_none() && app.diff_preview_lines.len() == 1 {
            return false;
        }
        app.diff_preview_cache_key = None;
        app.diff_preview_cache_hash = 0;
        app.diff_preview_lines = vec![Line::from("No diffs")];
        return true;
    };

    let mut hasher = DefaultHasher::new();
    selected.key.hash(&mut hasher);
    app.diff_theme.hash(&mut hasher);
    app.diff_wrap.hash(&mut hasher);
    width.hash(&mut hasher);

    const MAX_DIFF_PREVIEW_LINES: usize = 20_000;

    if selected.key == DIFF_ALL_KEY {
        let entries = app.diff_store.get("entries").and_then(|v| v.as_object());

        for row in rows.iter().skip(1) {
            row.key.hash(&mut hasher);
            row.content_omitted.hash(&mut hasher);
            row.additions.unwrap_or(0).hash(&mut hasher);
            row.deletions.unwrap_or(0).hash(&mut hasher);

            let Some(content) = entries
                .and_then(|e| e.get(&row.key))
                .and_then(|v| v.get("content"))
            else {
                0usize.hash(&mut hasher);
                continue;
            };

            let omitted = content
                .get("contentOmitted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            omitted.hash(&mut hasher);

            let old = content
                .get("oldContent")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new = content
                .get("newContent")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            hash_text_sample(&mut hasher, old);
            hash_text_sample(&mut hasher, new);
        }

        let content_hash = hasher.finish();
        if app.diff_preview_cache_key.as_deref() == Some(DIFF_ALL_KEY)
            && app.diff_preview_cache_hash == content_hash
            && app.diff_preview_cache_width == width_u16
        {
            return false;
        }

        app.diff_preview_cache_key = Some(DIFF_ALL_KEY.to_string());
        app.diff_preview_cache_hash = content_hash;

        let mut lines: Vec<Line<'static>> = vec![];
        if let Some(entries) = entries {
            for (idx, row) in rows.iter().enumerate().skip(1) {
                if idx > 1 && !lines.is_empty() {
                    lines.push(Line::from(""));
                }

                let entry_content = entries
                    .get(&row.key)
                    .and_then(|v| v.get("content"))
                    .cloned();
                let Some(content) = entry_content else {
                    lines.push(Line::from(format!("{} (missing diff content)", row.key)));
                    continue;
                };

                let omitted = content
                    .get("contentOmitted")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if omitted {
                    let adds = content
                        .get("additions")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let dels = content
                        .get("deletions")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    lines.push(Line::from(format!(
                        "{} (content omitted)  +{}/-{}",
                        row.key, adds, dels
                    )));
                    continue;
                }

                let old = content
                    .get("oldContent")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let new = content
                    .get("newContent")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let diff = utils::diff::create_unified_diff(&row.key, old, new);
                let highlight_path = row
                    .new_path
                    .as_deref()
                    .or(row.old_path.as_deref())
                    .unwrap_or(&row.key);
                lines.extend(highlight_unified_diff(
                    highlight_path,
                    &diff,
                    width,
                    app.diff_theme,
                    app.diff_wrap,
                ));

                if lines.len() > MAX_DIFF_PREVIEW_LINES {
                    lines.truncate(MAX_DIFF_PREVIEW_LINES);
                    lines.push(Line::from(Span::styled(
                        "… (truncated)".to_string(),
                        Style::default().add_modifier(Modifier::DIM),
                    )));
                    break;
                }
            }
        }

        if lines.is_empty() {
            lines.push(Line::from("No diffs"));
        }
        app.diff_preview_lines = lines;
        return true;
    }

    let entry_content = app
        .diff_store
        .get("entries")
        .and_then(|v| v.get(&selected.key))
        .and_then(|v| v.get("content"))
        .cloned();

    let omitted = entry_content
        .as_ref()
        .and_then(|c| c.get("contentOmitted"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    omitted.hash(&mut hasher);

    let (old, new, adds, dels) = if let Some(content) = entry_content.as_ref() {
        let old = content
            .get("oldContent")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let new = content
            .get("newContent")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let adds = content
            .get("additions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let dels = content
            .get("deletions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        (old, new, adds, dels)
    } else {
        ("", "", 0, 0)
    };

    hash_text_sample(&mut hasher, old);
    hash_text_sample(&mut hasher, new);
    adds.hash(&mut hasher);
    dels.hash(&mut hasher);
    let content_hash = hasher.finish();

    if app.diff_preview_cache_key.as_deref() == Some(&selected.key)
        && app.diff_preview_cache_hash == content_hash
        && app.diff_preview_cache_width == width_u16
    {
        return false;
    }

    app.diff_preview_cache_key = Some(selected.key.clone());
    app.diff_preview_cache_hash = content_hash;

    let mut lines: Vec<Line<'static>> = vec![];
    if entry_content.is_none() {
        lines.push(Line::from("No diff content"));
        app.diff_preview_lines = lines;
        return true;
    }

    let content = entry_content.unwrap();
    if omitted {
        let adds = content
            .get("additions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let dels = content
            .get("deletions")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        lines.push(Line::from(format!(
            "{} (content omitted)  +{}/-{}",
            selected.key, adds, dels
        )));
        app.diff_preview_lines = lines;
        return true;
    }

    let old = content
        .get("oldContent")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let new = content
        .get("newContent")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let diff = utils::diff::create_unified_diff(&selected.key, old, new);
    let highlight_path = selected
        .new_path
        .as_deref()
        .or(selected.old_path.as_deref())
        .unwrap_or(&selected.key);
    app.diff_preview_lines =
        highlight_unified_diff(highlight_path, &diff, width, app.diff_theme, app.diff_wrap);
    if app.diff_preview_lines.is_empty() {
        app.diff_preview_lines = vec![Line::from("No diff content")];
    }
    true
}

fn syntect_syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn syntect_theme_set() -> &'static ThemeSet {
    static SET: OnceLock<ThemeSet> = OnceLock::new();
    SET.get_or_init(ThemeSet::load_defaults)
}

fn syntect_fallback_theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(Theme::default)
}

fn syntect_theme(theme: DiffTheme) -> &'static Theme {
    let ts = syntect_theme_set();
    ts.themes
        .get(theme.syntect_key())
        .or_else(|| ts.themes.get(DiffTheme::default().syntect_key()))
        .or_else(|| ts.themes.values().next())
        .unwrap_or_else(|| syntect_fallback_theme())
}

fn syntax_for_path<'a>(ps: &'a SyntaxSet, path: &str) -> &'a SyntaxReference {
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path);

    if file_name == "Dockerfile" {
        if let Some(s) = ps.find_syntax_by_name("Dockerfile") {
            return s;
        }
    }
    if file_name == "Makefile" {
        if let Some(s) = ps.find_syntax_by_name("Makefile") {
            return s;
        }
    }

    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str());
    if let Some(ext) = ext {
        if let Some(syntax) = ps.find_syntax_by_extension(ext) {
            return syntax;
        }
        // Common aliases
        if ext == "rs" {
            if let Some(syntax) = ps.find_syntax_by_extension("rust") {
                return syntax;
            }
        }
        if ext == "yml" {
            if let Some(syntax) = ps.find_syntax_by_extension("yaml") {
                return syntax;
            }
        }
    }

    ps.find_syntax_plain_text()
}

fn syntect_style_to_ratatui(style: syntect::highlighting::Style) -> Style {
    let fg = style.foreground;
    Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b))
}

fn color_to_rgb(c: Color) -> Option<(u8, u8, u8)> {
    match c {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        Color::Black => Some((0, 0, 0)),
        Color::Red => Some((205, 49, 49)),
        Color::Green => Some((13, 188, 121)),
        Color::Yellow => Some((229, 229, 16)),
        Color::Blue => Some((36, 114, 200)),
        Color::Magenta => Some((188, 63, 188)),
        Color::Cyan => Some((17, 168, 205)),
        Color::Gray => Some((204, 204, 204)),
        Color::DarkGray => Some((118, 118, 118)),
        Color::LightRed => Some((241, 76, 76)),
        Color::LightGreen => Some((35, 209, 139)),
        Color::LightYellow => Some((245, 245, 67)),
        Color::LightBlue => Some((59, 142, 234)),
        Color::LightMagenta => Some((214, 112, 214)),
        Color::LightCyan => Some((41, 184, 219)),
        Color::White => Some((255, 255, 255)),
        _ => None,
    }
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance(rgb: (u8, u8, u8)) -> f64 {
    let (r, g, b) = rgb;
    let r = srgb_to_linear(r as f64 / 255.0);
    let g = srgb_to_linear(g as f64 / 255.0);
    let b = srgb_to_linear(b as f64 / 255.0);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (l1, l2) = if la >= lb { (la, lb) } else { (lb, la) };
    (l1 + 0.05) / (l2 + 0.05)
}

fn best_contrast_bw(bg: (u8, u8, u8)) -> Color {
    let black = (0, 0, 0);
    let white = (255, 255, 255);
    if contrast_ratio(white, bg) >= contrast_ratio(black, bg) {
        Color::White
    } else {
        Color::Black
    }
}

fn truncate_spans_to_width(mut spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    if width == 0 {
        return vec![];
    }

    let mut out: Vec<Span<'static>> = vec![];
    let mut used = 0usize;
    for span in spans.drain(..) {
        let w = display_width(span.content.as_ref());
        if used + w <= width {
            used += w;
            out.push(span);
            continue;
        }

        let remaining = width.saturating_sub(used);
        if remaining == 0 {
            break;
        }

        let truncated = truncate_to_width(span.content.as_ref(), remaining);
        out.push(Span::styled(truncated, span.style));
        break;
    }
    out
}

fn split_spans_by_width(
    spans: &[Span<'static>],
    max: usize,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    if max == 0 || spans.is_empty() {
        return (vec![], spans.to_vec());
    }

    let mut left: Vec<Span<'static>> = vec![];
    let mut remaining = max;
    for (i, span) in spans.iter().enumerate() {
        let w = display_width(span.content.as_ref());
        if w <= remaining {
            left.push(span.clone());
            remaining -= w;
            if remaining == 0 {
                return (left, spans[i + 1..].to_vec());
            }
            continue;
        }

        if remaining == 0 {
            return (left, spans[i..].to_vec());
        }

        let (chunk, rest) = split_by_width(span.content.as_ref(), remaining);
        if !chunk.is_empty() {
            left.push(Span::styled(chunk, span.style));
        }
        let mut right: Vec<Span<'static>> = vec![];
        if !rest.is_empty() {
            right.push(Span::styled(rest, span.style));
        }
        right.extend_from_slice(&spans[i + 1..]);
        return (left, right);
    }

    (left, vec![])
}

fn wrap_spans_hard(mut spans: Vec<Span<'static>>, width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let mut out: Vec<Vec<Span<'static>>> = vec![];
    while !spans.is_empty() {
        let total: usize = spans
            .iter()
            .map(|s| display_width(s.content.as_ref()))
            .sum();
        if total <= width {
            out.push(spans);
            break;
        }

        let (left, right) = split_spans_by_width(&spans, width);
        if left.is_empty() {
            // Ensure progress even with extremely small widths / wide chars.
            let first = spans.remove(0);
            out.push(vec![first]);
            continue;
        }
        out.push(left);
        spans = right;
    }
    out
}

fn highlight_unified_diff(
    file_path: &str,
    diff: &str,
    width: usize,
    theme: DiffTheme,
    wrap: bool,
) -> Vec<Line<'static>> {
    let ps = syntect_syntax_set();
    let syntect_theme = syntect_theme(theme);
    let syntax = syntax_for_path(ps, file_path);

    let mut old_hl = HighlightLines::new(syntax, syntect_theme);
    let mut new_hl = HighlightLines::new(syntax, syntect_theme);

    let mut out: Vec<Line<'static>> = vec![];
    for raw_line in diff.lines() {
        if raw_line.starts_with("--- ") || raw_line.starts_with("+++ ") {
            let spans = vec![Span::styled(
                raw_line.to_string(),
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            )];
            if wrap {
                for row in wrap_spans_hard(spans, width) {
                    out.push(Line::from(row));
                }
            } else {
                out.push(Line::from(truncate_spans_to_width(spans, width)));
            }
            continue;
        }

        if raw_line.starts_with("@@") {
            let spans = vec![Span::styled(
                raw_line.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )];
            if wrap {
                for row in wrap_spans_hard(spans, width) {
                    out.push(Line::from(row));
                }
            } else {
                out.push(Line::from(truncate_spans_to_width(spans, width)));
            }
            continue;
        }

        let (marker, rest) = raw_line.split_at(1.min(raw_line.len()));
        let marker_ch = marker.chars().next().unwrap_or(' ');
        let (gutter_style, marker_style) = match marker_ch {
            '+' => (
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            '-' => (
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            ' ' => (
                Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                Style::default().fg(Color::Gray),
            ),
            _ => (
                Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                Style::default().fg(Color::Gray),
            ),
        };

        let mut spans: Vec<Span<'static>> = vec![
            Span::styled("▌".to_string(), gutter_style),
            Span::styled(marker.to_string(), marker_style),
        ];
        let rest_spans = match marker_ch {
            '+' => new_hl
                .highlight_line(rest, ps)
                .map(|ranges| {
                    ranges
                        .into_iter()
                        .map(|(s, t)| Span::styled(t.to_string(), syntect_style_to_ratatui(s)))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|_| vec![Span::raw(rest.to_string())]),
            '-' => old_hl
                .highlight_line(rest, ps)
                .map(|ranges| {
                    ranges
                        .into_iter()
                        .map(|(s, t)| Span::styled(t.to_string(), syntect_style_to_ratatui(s)))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|_| vec![Span::raw(rest.to_string())]),
            ' ' => {
                // Advance both sides for better multi-line state.
                let _ = new_hl.highlight_line(rest, ps);
                old_hl
                    .highlight_line(rest, ps)
                    .map(|ranges| {
                        ranges
                            .into_iter()
                            .map(|(s, t)| Span::styled(t.to_string(), syntect_style_to_ratatui(s)))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|_| vec![Span::raw(rest.to_string())])
            }
            _ => vec![Span::raw(rest.to_string())],
        };
        spans.extend(rest_spans);

        // Semi-transparent GitHub-like backgrounds for additions/removals:
        // - added: hsl(138 69% 45% / .4)
        // - removed: hsl(5 100% 69% / .4)
        //
        // Terminals don't support alpha backgrounds, so we approximate by blending the HSL color
        // against the theme's "likely" background (white for light themes; #1e1e1e for dark).
        let line_bg = {
            fn blend_2_5(fg: u8, bg: u8) -> u8 {
                // 0.4*fg + 0.6*bg == (2/5)*fg + (3/5)*bg
                let v = (fg as u16) * 2 + (bg as u16) * 3;
                ((v + 2) / 5) as u8
            }

            let (bg_r, bg_g, bg_b) = if theme.is_light() {
                (255u8, 255u8, 255u8)
            } else {
                (30u8, 30u8, 30u8)
            };

            // Precomputed from HSL:
            // - hsl(138 69% 45%) => rgb(36, 194, 83)
            // - hsl(5 100% 69%)  => rgb(255, 110, 97)
            if marker_ch == '+' {
                Some(Color::Rgb(
                    blend_2_5(36, bg_r),
                    blend_2_5(194, bg_g),
                    blend_2_5(83, bg_b),
                ))
            } else if marker_ch == '-' {
                Some(Color::Rgb(
                    blend_2_5(255, bg_r),
                    blend_2_5(110, bg_g),
                    blend_2_5(97, bg_b),
                ))
            } else {
                None
            }
        };

        if let Some(bg) = line_bg {
            for s in spans.iter_mut() {
                s.style = s.style.bg(bg);
            }
        }

        let apply_contrast = |spans: &mut [Span<'static>], bg: Color, skip: usize| {
            if let Some(bg_rgb) = color_to_rgb(bg) {
                const MIN_CONTRAST: f64 = 3.0;
                for span in spans.iter_mut().skip(skip) {
                    let Some(fg) = span.style.fg else {
                        continue;
                    };
                    let Some(fg_rgb) = color_to_rgb(fg) else {
                        continue;
                    };
                    if contrast_ratio(fg_rgb, bg_rgb) < MIN_CONTRAST {
                        span.style = span.style.fg(best_contrast_bw(bg_rgb));
                    }
                }
            }
        };

        let pad_bg = |spans: &mut Vec<Span<'static>>, bg: Color| {
            let used = spans
                .iter()
                .map(|s| display_width(s.content.as_ref()))
                .sum::<usize>();
            if used < width {
                spans.push(Span::styled(
                    " ".repeat(width - used),
                    Style::default().bg(bg),
                ));
            }
        };

        if wrap && width > 2 {
            let content_width = width.saturating_sub(2).max(1);
            let content = spans.split_off(2);
            let wrapped = wrap_spans_hard(content, content_width);

            for (i, chunk) in wrapped.into_iter().enumerate() {
                let mut line_spans: Vec<Span<'static>> = if i == 0 {
                    spans.clone()
                } else {
                    let mut p = vec![
                        Span::styled("▌".to_string(), gutter_style),
                        Span::styled(" ".to_string(), marker_style),
                    ];
                    if let Some(bg) = line_bg {
                        for s in p.iter_mut() {
                            s.style = s.style.bg(bg);
                        }
                    }
                    p
                };
                line_spans.extend(chunk);
                if let Some(bg) = line_bg {
                    apply_contrast(&mut line_spans, bg, 2);
                    pad_bg(&mut line_spans, bg);
                }
                out.push(Line::from(line_spans));
            }
        } else {
            let mut spans = truncate_spans_to_width(spans, width);
            if let Some(bg) = line_bg {
                apply_contrast(&mut spans, bg, 2);
                pad_bg(&mut spans, bg);
            }
            out.push(Line::from(spans));
        }
    }
    out
}

fn split_by_width(s: &str, max: usize) -> (String, String) {
    if max == 0 {
        return (String::new(), s.to_string());
    }
    let mut chunk = String::new();
    let mut last_byte = 0usize;
    for (i, ch) in s.char_indices() {
        let next = format!("{chunk}{ch}");
        if display_width(&next) > max {
            break;
        }
        chunk.push(ch);
        last_byte = i + ch.len_utf8();
    }
    let rest = s.get(last_byte..).unwrap_or("").to_string();
    (chunk, rest)
}

fn push_span_merged(spans: &mut Vec<Span<'static>>, text: String, style: Style) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.style == style
    {
        last.content.to_mut().push_str(&text);
        return;
    }
    spans.push(Span::styled(text, style));
}

fn wrap_md_tokens(
    tokens: &[MdToken],
    width: usize,
    prefix_first: &str,
    prefix_next: &str,
    prefix_style: Style,
) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut out: Vec<Line<'static>> = vec![];

    let mut cur_spans: Vec<Span<'static>> = vec![];
    let mut cur_w: usize = 0;
    let mut cur_prefix_w: usize = 0;

    let start_line = |spans: &mut Vec<Span<'static>>,
                      cur_w: &mut usize,
                      cur_prefix_w: &mut usize,
                      first: bool| {
        spans.clear();
        let prefix = if first { prefix_first } else { prefix_next };
        if !prefix.is_empty() {
            spans.push(Span::styled(prefix.to_string(), prefix_style));
            *cur_prefix_w = display_width(prefix);
            *cur_w = *cur_prefix_w;
        } else {
            *cur_prefix_w = 0;
            *cur_w = 0;
        }
    };

    start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, true);

    for token in tokens.iter().cloned() {
        match token {
            MdToken::Newline => {
                out.push(Line::from(cur_spans.clone()));
                start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, false);
            }
            MdToken::Text(mut text, style) => {
                if text == " " && cur_w == cur_prefix_w {
                    continue;
                }
                loop {
                    let available = width.saturating_sub(cur_w);
                    if available == 0 {
                        out.push(Line::from(cur_spans.clone()));
                        start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, false);
                        continue;
                    }

                    let w = display_width(&text);
                    if w <= available {
                        push_span_merged(&mut cur_spans, text, style);
                        cur_w += w;
                        break;
                    }

                    // Prefer word wrapping: if this is a non-space token and we're not at the
                    // beginning of the line, move it to the next line instead of splitting it.
                    if text != " " && cur_w > cur_prefix_w {
                        out.push(Line::from(cur_spans.clone()));
                        start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, false);
                        continue;
                    }

                    // If the token is too long even at the line start, hard-split by width.
                    let (chunk, rest) = split_by_width(&text, available);
                    if chunk.is_empty() {
                        // Should be rare; avoid infinite loops.
                        out.push(Line::from(cur_spans.clone()));
                        start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, false);
                        continue;
                    }
                    let chunk_w = display_width(&chunk);
                    push_span_merged(&mut cur_spans, chunk, style);
                    cur_w += chunk_w;
                    out.push(Line::from(cur_spans.clone()));
                    start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, false);
                    text = rest;
                    if text.is_empty() {
                        break;
                    }
                }
            }
        }
    }

    if !cur_spans.is_empty() {
        out.push(Line::from(cur_spans));
    }

    // Trim trailing empty lines.
    while out.last().is_some_and(|l| l.spans.is_empty()) {
        out.pop();
    }

    out
}

fn render_markdown(md: &str, width: usize, softbreak_mode: MdSoftBreakMode) -> Vec<Line<'static>> {
    let md = sanitize_tui_text(md);
    let width = width.max(1);

    let mut options = MdOptions::empty();
    options.insert(MdOptions::ENABLE_STRIKETHROUGH);
    options.insert(MdOptions::ENABLE_TABLES);
    options.insert(MdOptions::ENABLE_TASKLISTS);

    let parser = MdParser::new_ext(md.as_ref(), options);

    #[derive(Debug, Clone, Copy)]
    struct ListCtx {
        ordered: bool,
        next_number: usize,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum BlockKind {
        Paragraph,
        Heading,
        Item,
    }

    struct Block {
        kind: BlockKind,
        tokens: Vec<MdToken>,
        prefix_first: String,
        prefix_next: String,
        prefix_style: Style,
        trailing_blank_line: bool,
    }

    let mut out: Vec<Line<'static>> = vec![];
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut quote_depth: usize = 0;
    let mut lists: Vec<ListCtx> = vec![];
    let mut block: Option<Block> = None;

    let mut in_code_block = false;
    let mut code_buf = String::new();

    let prefix_style = Style::default().fg(Color::Gray);

    fn base_prefix(quote_depth: usize, list_depth: usize) -> String {
        let mut p = String::new();
        if quote_depth > 0 {
            p.push_str(&"│ ".repeat(quote_depth));
        }
        if list_depth > 1 {
            p.push_str(&"  ".repeat(list_depth - 1));
        }
        p
    }

    let push_word = |block: &mut Block, word: &str, style: Style| {
        if let Some(MdToken::Text(prev, _)) = block.tokens.last() {
            if !prev.is_empty() && !prev.ends_with(' ') {
                block.tokens.push(MdToken::Text(" ".to_string(), style));
            }
        }
        block.tokens.push(MdToken::Text(word.to_string(), style));
    };

    let push_text = |block: &mut Block, text: &str, style: Style| {
        for word in text.split_whitespace() {
            push_word(block, word, style);
        }
    };

    let flush_block = |out: &mut Vec<Line<'static>>, block: &mut Option<Block>| {
        let Some(b) = block.take() else {
            return;
        };
        if b.tokens.is_empty() {
            return;
        }
        let lines = wrap_md_tokens(
            &b.tokens,
            width,
            &b.prefix_first,
            &b.prefix_next,
            b.prefix_style,
        );
        out.extend(lines);
        if b.trailing_blank_line {
            out.push(Line::from(""));
        }
    };

    for event in parser {
        if in_code_block {
            match event {
                MdEvent::End(MdTagEnd::CodeBlock) => {
                    let code_style = Style::default().bg(Color::DarkGray);
                    for l in code_buf.lines() {
                        out.push(Line::from(Span::styled(
                            truncate_to_width(l, width),
                            code_style,
                        )));
                    }
                    code_buf.clear();
                    in_code_block = false;
                    out.push(Line::from(""));
                }
                MdEvent::Text(t) | MdEvent::Code(t) => code_buf.push_str(&t),
                MdEvent::SoftBreak | MdEvent::HardBreak => code_buf.push('\n'),
                _ => {}
            }
            continue;
        }

        match event {
            MdEvent::Start(MdTag::Paragraph) => {
                if block.as_ref().is_some_and(|b| b.kind == BlockKind::Item) {
                    continue;
                }
                flush_block(&mut out, &mut block);
                let base = base_prefix(quote_depth, lists.len());
                block = Some(Block {
                    kind: BlockKind::Paragraph,
                    tokens: vec![],
                    prefix_first: base.clone(),
                    prefix_next: base,
                    prefix_style,
                    trailing_blank_line: true,
                });
            }
            MdEvent::End(MdTagEnd::Paragraph) => {
                if block.as_ref().is_some_and(|b| b.kind == BlockKind::Item) {
                    continue;
                }
                flush_block(&mut out, &mut block);
            }
            MdEvent::Start(MdTag::Heading { .. }) => {
                flush_block(&mut out, &mut block);
                let base = base_prefix(quote_depth, lists.len());
                block = Some(Block {
                    kind: BlockKind::Heading,
                    tokens: vec![],
                    prefix_first: base.clone(),
                    prefix_next: base,
                    prefix_style,
                    trailing_blank_line: true,
                });
                let h_style = Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan);
                style_stack.push(h_style);
            }
            MdEvent::End(MdTagEnd::Heading(_)) => {
                let _ = style_stack.pop();
                flush_block(&mut out, &mut block);
            }
            MdEvent::Start(MdTag::BlockQuote) => {
                quote_depth += 1;
            }
            MdEvent::End(MdTagEnd::BlockQuote) => {
                quote_depth = quote_depth.saturating_sub(1);
            }
            MdEvent::Start(MdTag::List(start)) => {
                let ordered = start.is_some();
                let next_number = start.unwrap_or(1) as usize;
                lists.push(ListCtx {
                    ordered,
                    next_number,
                });
            }
            MdEvent::End(MdTagEnd::List(_)) => {
                let _ = lists.pop();
                // A list boundary is a decent place to add separation.
                out.push(Line::from(""));
            }
            MdEvent::Start(MdTag::Item) => {
                flush_block(&mut out, &mut block);
                let base = base_prefix(quote_depth, lists.len());
                let bullet = if let Some(list) = lists.last_mut() {
                    if list.ordered {
                        let b = format!("{}. ", list.next_number);
                        list.next_number += 1;
                        b
                    } else {
                        "- ".to_string()
                    }
                } else {
                    "- ".to_string()
                };
                let cont = " ".repeat(display_width(&bullet));
                block = Some(Block {
                    kind: BlockKind::Item,
                    tokens: vec![],
                    prefix_first: format!("{base}{bullet}"),
                    prefix_next: format!("{base}{cont}"),
                    prefix_style,
                    trailing_blank_line: false,
                });
            }
            MdEvent::End(MdTagEnd::Item) => {
                flush_block(&mut out, &mut block);
            }
            MdEvent::Start(MdTag::CodeBlock(CodeBlockKind::Fenced(_)))
            | MdEvent::Start(MdTag::CodeBlock(CodeBlockKind::Indented)) => {
                flush_block(&mut out, &mut block);
                in_code_block = true;
                code_buf.clear();
            }
            MdEvent::Start(MdTag::Emphasis) => {
                let next = style_stack
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .add_modifier(Modifier::ITALIC);
                style_stack.push(next);
            }
            MdEvent::End(MdTagEnd::Emphasis) => {
                let _ = style_stack.pop();
            }
            MdEvent::Start(MdTag::Strong) => {
                let next = style_stack
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .add_modifier(Modifier::BOLD);
                style_stack.push(next);
            }
            MdEvent::End(MdTagEnd::Strong) => {
                let _ = style_stack.pop();
            }
            MdEvent::Start(MdTag::Link { .. }) => {
                let next = style_stack
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .add_modifier(Modifier::UNDERLINED);
                style_stack.push(next);
            }
            MdEvent::End(MdTagEnd::Link) => {
                let _ = style_stack.pop();
            }
            MdEvent::Code(t) => {
                let code_style = style_stack
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .fg(Color::Yellow);
                if block.is_none() {
                    let base = base_prefix(quote_depth, lists.len());
                    block = Some(Block {
                        kind: BlockKind::Paragraph,
                        tokens: vec![],
                        prefix_first: base.clone(),
                        prefix_next: base,
                        prefix_style,
                        trailing_blank_line: true,
                    });
                }
                if let Some(b) = block.as_mut() {
                    if let Some(MdToken::Text(prev, _)) = b.tokens.last() {
                        if !prev.is_empty() && !prev.ends_with(' ') {
                            b.tokens
                                .push(MdToken::Text(" ".to_string(), Style::default()));
                        }
                    }
                    // Treat inline code like normal text for wrapping purposes (but keep style).
                    for word in t.split_whitespace() {
                        push_word(b, word, code_style);
                    }
                }
            }
            MdEvent::Text(t) => {
                let style = style_stack.last().copied().unwrap_or_default();
                if block.is_none() {
                    let base = base_prefix(quote_depth, lists.len());
                    block = Some(Block {
                        kind: BlockKind::Paragraph,
                        tokens: vec![],
                        prefix_first: base.clone(),
                        prefix_next: base,
                        prefix_style,
                        trailing_blank_line: true,
                    });
                }
                if let Some(b) = block.as_mut() {
                    push_text(b, &t, style);
                }
            }
            MdEvent::SoftBreak => match softbreak_mode {
                MdSoftBreakMode::Space => {
                    if let Some(b) = block.as_mut() {
                        b.tokens
                            .push(MdToken::Text(" ".to_string(), Style::default()));
                    }
                }
                MdSoftBreakMode::Newline => {
                    if let Some(b) = block.as_mut() {
                        b.tokens.push(MdToken::Newline);
                    }
                }
            },
            MdEvent::HardBreak => {
                if let Some(b) = block.as_mut() {
                    b.tokens.push(MdToken::Newline);
                }
            }
            _ => {}
        }
    }

    flush_block(&mut out, &mut block);

    while out.last().is_some_and(|l| l.spans.is_empty()) {
        out.pop();
    }

    out
}

fn short_time(iso: &str) -> Option<&str> {
    // Best-effort extraction of "HH:MM:SS" from RFC3339 timestamps.
    // Example: "2026-01-02T09:31:00.123Z" -> "09:31:00"
    let t = iso.split('T').nth(1)?;
    let time = t.split(['.', 'Z', '+', '-']).next()?;
    if time.len() >= 8 {
        Some(&time[..8])
    } else {
        Some(time)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

fn contains_ci(haystack: &str, needle: &str) -> bool {
    let needle = needle.trim();
    if needle.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

#[derive(Debug, Clone)]
struct ProjectRow {
    id: Uuid,
    name: String,
}

fn filtered_projects(app: &AppState) -> Vec<ProjectRow> {
    let mut list = projects_list(&app.projects_store);
    let q = app.project_filter.trim();
    if !q.is_empty() {
        list.retain(|p| contains_ci(&p.name, q));
    }
    list
}

fn projects_list(store: &serde_json::Value) -> Vec<ProjectRow> {
    let projects_obj = store.get("projects").and_then(|v| v.as_object());
    let Some(projects_obj) = projects_obj else {
        return vec![];
    };

    let mut rows = Vec::with_capacity(projects_obj.len());
    for (id_str, project) in projects_obj.iter() {
        let Ok(id) = Uuid::parse_str(id_str) else {
            continue;
        };
        let name = project
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| project.get("title").and_then(|v| v.as_str()))
            .unwrap_or("(unnamed)")
            .to_string();
        rows.push(ProjectRow { id, name });
    }

    rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    rows
}

fn exec_list(store: &serde_json::Value) -> Vec<ExecRow> {
    let exec_obj = store.get("execution_processes").and_then(|v| v.as_object());
    let Some(exec_obj) = exec_obj else {
        return vec![];
    };

    let mut rows = Vec::with_capacity(exec_obj.len());
    for (id_str, exec) in exec_obj.iter() {
        let Ok(id) = Uuid::parse_str(id_str) else {
            continue;
        };
        let session_id = exec
            .get("session_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        let run_reason = exec
            .get("run_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let status = exec
            .get("status")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let created_at = exec
            .get("created_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let dropped = exec
            .get("dropped")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        rows.push(ExecRow {
            id,
            session_id,
            run_reason,
            status,
            created_at,
            dropped,
        });
    }

    rows.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    rows
}

fn active_exec_id(execs: &[ExecRow]) -> Option<Uuid> {
    let mut filtered: Vec<&ExecRow> = execs.iter().filter(|e| !e.dropped).collect();
    if filtered.is_empty() {
        return None;
    }
    // exec_list is sorted oldest -> newest, keep that invariant for selection.
    filtered.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let running: Vec<&ExecRow> = filtered
        .iter()
        .copied()
        .filter(|e| e.status.as_deref() == Some("running"))
        .collect();
    if !running.is_empty() {
        if let Some(non_dev) = running
            .iter()
            .copied()
            .find(|e| e.run_reason.as_deref() != Some("dev_server"))
        {
            return Some(non_dev.id);
        }
        return Some(running[running.len() - 1].id);
    }

    if let Some(agent) = filtered
        .iter()
        .rev()
        .copied()
        .find(|e| e.run_reason.as_deref() == Some("coding_agent"))
    {
        return Some(agent.id);
    }

    Some(filtered[filtered.len() - 1].id)
}

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

fn set_selected_project(app: &mut AppState, project_id: Option<Uuid>) {
    if app.selected_project_id == project_id {
        return;
    }
    app.selected_project_id = project_id;
    let _ = app.project_sel_tx.send(project_id);
    app.prefs.selected_project_id = project_id;
    save_prefs(&app.prefs);
    // Project switch invalidates task/attempt/exec/log selections immediately.
    set_selected_task(app, None);
}

fn set_selected_task(app: &mut AppState, task_id: Option<Uuid>) {
    if app.selected_task_id == task_id {
        return;
    }

    app.selected_task_id = task_id;

    // Clear dependent panes.
    app.attempts.clear();
    app.selected_attempt_index = 0;
    set_selected_attempt(app, None);

    // Fetch attempts for the new task selection.
    if let Some(task_id) = task_id {
        let base_url = app.backend_url.clone();
        let net_tx = app.net_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            load_attempts_task(base_url, task_id, net_tx).await;
        });
    }
}

fn set_selected_attempt(app: &mut AppState, attempt_id: Option<Uuid>) {
    if app.selected_attempt_id == attempt_id {
        return;
    }

    app.selected_attempt_id = attempt_id;

    app.exec_store = serde_json::json!({ "execution_processes": {} });
    set_selected_exec(app, None);

    app.diff_store = serde_json::json!({ "entries": {} });
    app.selected_diff_index = 0;
    app.diff_scroll_offset = 0;

    app.repo_statuses.clear();
    app.selected_repo_index = 0;

    let _ = app.attempt_sel_tx.send(attempt_id);
    request_branch_status_refresh(app);
}

fn set_selected_exec(app: &mut AppState, exec_id: Option<Uuid>) {
    if app.selected_exec_id == exec_id {
        return;
    }

    app.selected_exec_id = exec_id;
    reset_logs(app);
    let _ = app.exec_sel_tx.send(exec_id);
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

fn request_branch_status_refresh(app: &mut AppState) {
    let Some(attempt_id) = app.selected_attempt_id else {
        return;
    };

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match branch_status_http(&base_url, attempt_id).await {
            Ok(statuses) => {
                let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("branch status failed: {e}")))
                    .await;
            }
        }
    });
}

fn submit_composer(app: &mut AppState) {
    let msg = app.composer_buffer.trim().to_string();
    if msg.is_empty() {
        app.composer_active = false;
        app.composer_buffer.clear();
        return;
    }

    app.composer_active = false;
    app.composer_buffer.clear();

    if msg.trim_start().starts_with('/') {
        submit_slash_command(app, &msg);
        return;
    }

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    let attempt_id = app.selected_attempt_id;

    let execs = exec_list(&app.exec_store);
    let active = app
        .selected_exec_id
        .and_then(|id| execs.iter().find(|e| e.id == id));
    let session_id = active.and_then(|e| e.session_id);
    let is_running = active.and_then(|e| e.status.as_deref()) == Some("running");

    tokio::spawn(async move {
        let session_id = match session_id {
            Some(id) => Some(id),
            None => match attempt_id {
                Some(workspace_id) => match latest_session_id_http(&base_url, workspace_id).await {
                    Ok(id) => id,
                    Err(e) => {
                        let _ = net_tx
                            .send(NetEvent::Error(format!("failed to load sessions: {e}")))
                            .await;
                        return;
                    }
                },
                None => None,
            },
        };

        let Some(session_id) = session_id else {
            let _ = net_tx
                .send(NetEvent::Error(
                    "no session available for this attempt".to_string(),
                ))
                .await;
            return;
        };

        let result = if is_running {
            queue_follow_up_http(&base_url, session_id, &msg).await
        } else {
            follow_up_http(&base_url, session_id, &msg).await
        };

        match result {
            Ok(()) => {}
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("follow-up failed: {e}")))
                    .await;
            }
        }
    });
}

fn submit_slash_command(app: &mut AppState, raw: &str) {
    let cmdline = raw.trim_start().trim_start_matches('/');
    let tokens = match tokenize_command_line(cmdline) {
        Ok(t) => t,
        Err(e) => {
            app.last_error = Some(format!("invalid command: {e}"));
            return;
        }
    };

    if tokens.is_empty() {
        app.last_error = Some("invalid command: empty".to_string());
        return;
    }

    match parse_slash_command(app, &tokens) {
        Ok(()) => {}
        Err(e) => {
            app.last_error = Some(e);
        }
    }
}

fn parse_slash_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    match tokens[0].as_str() {
        "help" | "?" => {
            app.show_help = true;
            app.last_notice = Some("Opened help. (Press Esc to close)".to_string());
            Ok(())
        }
        "status" => {
            request_branch_status_refresh(app);
            app.last_notice = Some("Refreshing branch status…".to_string());
            Ok(())
        }
        "repo" => handle_repo_command(app, tokens.get(1).map(|s| s.as_str())),
        "rebase" => handle_rebase_command(app, tokens),
        "abort" => handle_abort_command(app, tokens),
        "merge" => handle_merge_command(app, tokens),
        "push" => handle_push_command(app, tokens),
        "pr" => handle_pr_command(app, tokens),
        "open" => handle_open_command(app, tokens),
        other => Err(format!("unknown command: /{other} (try /help)")),
    }
}

fn handle_abort_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let mut repo_arg: Option<String> = None;
    let mut i = 1;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

    let attempt_id = app
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match abort_conflicts_http(&base_url, attempt_id, repo_id).await {
            Ok(()) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!(
                        "Aborted conflicts for {repo_name}."
                    )))
                    .await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                }
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("abort failed: {e}")))
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_repo_command(app: &mut AppState, arg: Option<&str>) -> Result<(), String> {
    if app.repo_statuses.is_empty() {
        request_branch_status_refresh(app);
        app.last_notice = Some("Loading repos…".to_string());
        return Ok(());
    }

    let Some(arg) = arg.filter(|s| !s.trim().is_empty()) else {
        let mut msg = String::new();
        msg.push_str("Repos:\n");
        for (idx, repo) in app.repo_statuses.iter().enumerate() {
            let marker = if idx == app.selected_repo_index {
                "*"
            } else {
                " "
            };
            msg.push_str(&format!("  {marker} {}. {}\n", idx + 1, repo.repo_name));
        }
        app.last_notice = Some(msg.trim_end().to_string());
        return Ok(());
    };

    let idx = if let Ok(n) = arg.parse::<usize>() {
        n.saturating_sub(1)
    } else {
        let needle = arg.to_ascii_lowercase();
        app.repo_statuses
            .iter()
            .position(|r| r.repo_name.to_ascii_lowercase() == needle)
            .or_else(|| {
                app.repo_statuses
                    .iter()
                    .position(|r| r.repo_name.to_ascii_lowercase().contains(&needle))
            })
            .ok_or_else(|| format!("unknown repo: {arg}"))?
    };

    if idx >= app.repo_statuses.len() {
        return Err(format!("repo index out of range: {arg}"));
    }
    app.selected_repo_index = idx;
    app.last_notice = Some(format!(
        "Selected repo: {}",
        app.repo_statuses[idx].repo_name
    ));
    Ok(())
}

fn handle_rebase_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let mut repo_arg: Option<String> = None;
    let mut onto: Option<String> = None;
    let mut old: Option<String> = None;

    let mut i = 1;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            "--onto" => {
                i += 1;
                onto = tokens.get(i).cloned();
            }
            "--old" => {
                i += 1;
                old = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

    let attempt_id = app
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match rebase_task_attempt_http(&base_url, attempt_id, repo_id, old, onto).await {
            Ok(()) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!("Rebase started for {repo_name}.")))
                    .await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                }
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("rebase failed: {e}")))
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_merge_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let mut repo_arg: Option<String> = None;
    let mut i = 1;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

    let attempt_id = app
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match merge_task_attempt_http(&base_url, attempt_id, repo_id).await {
            Ok(()) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!("Merged {repo_name}.")))
                    .await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                }
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("merge failed: {e}")))
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_push_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let mut repo_arg: Option<String> = None;
    let mut force = false;

    let mut i = 1;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            "--force" => force = true,
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

    let attempt_id = app
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        let result = if force {
            force_push_task_attempt_branch_http(&base_url, attempt_id, repo_id).await
        } else {
            push_task_attempt_branch_http(&base_url, attempt_id, repo_id).await
        };
        match result {
            Ok(()) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!(
                        "Pushed {repo_name}{}.",
                        if force { " (force)" } else { "" }
                    )))
                    .await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                }
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("push failed: {e}")))
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_pr_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    if tokens.len() < 2 {
        return Err("usage: /pr <create|attach|comments>".to_string());
    }
    match tokens[1].as_str() {
        "create" => handle_pr_create_command(app, tokens),
        "attach" => handle_pr_attach_command(app, tokens),
        "comments" => handle_pr_comments_command(app, tokens),
        other => Err(format!("unknown subcommand: pr {other}")),
    }
}

fn handle_pr_create_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let mut repo_arg: Option<String> = None;
    let mut title: Option<String> = None;
    let mut body: Option<String> = None;
    let mut base: Option<String> = None;
    let mut draft: Option<bool> = None;
    let mut auto_desc = false;

    let mut i = 2;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            "--title" => {
                i += 1;
                title = tokens.get(i).cloned();
            }
            "--body" => {
                i += 1;
                body = tokens.get(i).cloned();
            }
            "--base" => {
                i += 1;
                base = tokens.get(i).cloned();
            }
            "--draft" => {
                draft = Some(true);
            }
            "--auto-desc" => {
                auto_desc = true;
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

    let attempt_id = app
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    let title = title
        .or_else(|| {
            app.selected_task_id
                .and_then(|id| find_task(&app.tasks_store, id).map(|t| t.title))
        })
        .ok_or_else(|| "missing --title and no task selected".to_string())?;

    let draft = draft.or(Some(false));

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match create_pr_http(
            &base_url,
            attempt_id,
            CreateGitHubPrRequest {
                title,
                body,
                target_branch: base,
                draft,
                repo_id,
                auto_generate_description: auto_desc,
            },
        )
        .await
        {
            Ok(url) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!(
                        "PR created for {repo_name}: {url}"
                    )))
                    .await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                }
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("pr create failed: {e}")))
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_pr_attach_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let mut repo_arg: Option<String> = None;
    let mut i = 2;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

    let attempt_id = app
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match attach_pr_http(&base_url, attempt_id, repo_id).await {
            Ok(resp) => {
                let msg = if resp.pr_attached {
                    if let Some(url) = resp.pr_url {
                        format!("Attached PR for {repo_name}: {url}")
                    } else {
                        format!("Attached PR for {repo_name}.")
                    }
                } else {
                    format!("No PR found to attach for {repo_name}.")
                };
                let _ = net_tx.send(NetEvent::Notice(msg)).await;
                if let Ok(statuses) = branch_status_http(&base_url, attempt_id).await {
                    let _ = net_tx.send(NetEvent::BranchStatusLoaded(statuses)).await;
                }
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("pr attach failed: {e}")))
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_pr_comments_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    let mut repo_arg: Option<String> = None;
    let mut i = 2;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "--repo" => {
                i += 1;
                repo_arg = tokens.get(i).cloned();
            }
            other => return Err(format!("unexpected arg: {other}")),
        }
        i += 1;
    }

    let attempt_id = app
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;
    let (repo_id, repo_name) = resolve_repo_for_command(app, repo_arg.as_deref())?;

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        match get_pr_comments_http(&base_url, attempt_id, repo_id).await {
            Ok(count) => {
                let _ = net_tx
                    .send(NetEvent::Notice(format!(
                        "Fetched {count} PR comments for {repo_name}."
                    )))
                    .await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("pr comments failed: {e}")))
                    .await;
            }
        }
    });
    Ok(())
}

fn handle_open_command(app: &mut AppState, tokens: &[String]) -> Result<(), String> {
    if tokens.len() < 2 {
        return Err("usage: /open <file_path>".to_string());
    }
    let attempt_id = app
        .selected_attempt_id
        .ok_or_else(|| "no attempt selected".to_string())?;

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    let file_path = tokens[1].clone();

    tokio::spawn(async move {
        match open_editor_http(&base_url, attempt_id, Some(file_path.clone())).await {
            Ok(url) => {
                let msg = match url {
                    Some(url) => format!("Opened editor for {file_path}: {url}"),
                    None => format!("Opened editor for {file_path}."),
                };
                let _ = net_tx.send(NetEvent::Notice(msg)).await;
            }
            Err(e) => {
                let _ = net_tx
                    .send(NetEvent::Error(format!("open editor failed: {e}")))
                    .await;
            }
        }
    });
    Ok(())
}

fn resolve_repo_for_command(
    app: &mut AppState,
    repo_arg: Option<&str>,
) -> Result<(Uuid, String), String> {
    if app.repo_statuses.is_empty() {
        request_branch_status_refresh(app);
        return Err("no repo status loaded yet (run /status)".to_string());
    }

    if let Some(arg) = repo_arg.filter(|s| !s.trim().is_empty()) {
        if let Ok(n) = arg.parse::<usize>() {
            let idx = n.saturating_sub(1);
            let repo = app
                .repo_statuses
                .get(idx)
                .ok_or_else(|| format!("repo index out of range: {arg}"))?;
            return Ok((repo.repo_id, repo.repo_name.clone()));
        }

        let needle = arg.to_ascii_lowercase();
        let idx = app
            .repo_statuses
            .iter()
            .position(|r| r.repo_name.to_ascii_lowercase() == needle)
            .or_else(|| {
                app.repo_statuses
                    .iter()
                    .position(|r| r.repo_name.to_ascii_lowercase().contains(&needle))
            })
            .ok_or_else(|| format!("unknown repo: {arg}"))?;
        app.selected_repo_index = idx;
    }

    let repo = app
        .repo_statuses
        .get(app.selected_repo_index)
        .ok_or_else(|| "no repo selected".to_string())?;
    Ok((repo.repo_id, repo.repo_name.clone()))
}

fn tokenize_command_line(s: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = vec![];
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    let mut quote: Option<char> = None;

    while let Some(ch) = chars.next() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                    continue;
                }
                if ch == '\\' && q == '"' {
                    if let Some(next) = chars.next() {
                        cur.push(next);
                    }
                    continue;
                }
                cur.push(ch);
            }
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                '\\' => {
                    if let Some(next) = chars.next() {
                        cur.push(next);
                    }
                }
                c if c.is_whitespace() => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    while matches!(chars.peek(), Some(p) if p.is_whitespace()) {
                        chars.next();
                    }
                }
                _ => cur.push(ch),
            },
        }
    }

    if quote.is_some() {
        return Err("unterminated quote".to_string());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

fn select_adjacent_attempt(app: &mut AppState, delta: i32) {
    if app.attempts.is_empty() {
        return;
    }

    let cur = app
        .selected_attempt_id
        .and_then(|id| app.attempts.iter().position(|a| a.id == id))
        .unwrap_or(app.selected_attempt_index.min(app.attempts.len() - 1));

    let next = clamp_index(cur, delta, app.attempts.len());
    if next == cur {
        return;
    }

    app.selected_attempt_index = next;
    let id = app.attempts.get(next).map(|a| a.id);
    set_selected_attempt(app, id);
}

fn move_active_status(app: &mut AppState, delta: i32) {
    let statuses = board_statuses(app);
    if statuses.is_empty() {
        return;
    }

    let cur = statuses
        .iter()
        .position(|s| *s == app.tasks_active_column)
        .unwrap_or(0);
    let next = clamp_index(cur, delta, statuses.len());
    app.tasks_active_column = statuses[next];
    ensure_selected_task_in_active_column(app);
}

fn select_adjacent_diff_file(app: &mut AppState, delta: i32) {
    let rows = diff_rows_with_all(&app.diff_store);
    if rows.is_empty() {
        app.selected_diff_index = 0;
        return;
    }

    let cur = app.selected_diff_index.min(rows.len() - 1);
    let next = clamp_index(cur, delta, rows.len());
    if next == cur {
        return;
    }

    app.selected_diff_index = next;
    app.diff_scroll_offset = 0;
    sync_selected_repo_from_diff_selection(app);
    schedule_diff_preview_refresh(app, Duration::from_millis(0));
}

fn ensure_selection_visible(app: &mut AppState) {
    ensure_project_selection(app);
    ensure_task_selection(app);
    ensure_attempt_selection(app);
    ensure_exec_selection(app);
}

fn ensure_project_selection(app: &mut AppState) {
    let projects = filtered_projects(app);
    if projects.is_empty() {
        app.selected_project_index = 0;
        set_selected_project(app, None);
        return;
    }

    if let Some(id) = app.selected_project_id
        && let Some(idx) = projects.iter().position(|p| p.id == id)
    {
        app.selected_project_index = idx;
        return;
    }

    app.selected_project_index = 0;
    set_selected_project(app, Some(projects[0].id));
}

fn ensure_task_selection(app: &mut AppState) {
    if app.selected_project_id.is_none() {
        set_selected_task(app, None);
        return;
    }

    let list = tasks_filtered_by_status(app, app.tasks_active_column);
    if list.is_empty() {
        set_selected_task(app, None);
        return;
    }

    if let Some(id) = app.selected_task_id
        && let Some(idx) = list.iter().position(|t| t.id == id)
    {
        app.board_index_by_status[app.tasks_active_column.idx()] = idx;
        return;
    }

    let idx = app.board_index_by_status[app.tasks_active_column.idx()].min(list.len() - 1);
    set_selected_task(app, Some(list[idx].id));
}

fn ensure_attempt_selection(app: &mut AppState) {
    if app.attempts.is_empty() {
        app.selected_attempt_index = 0;
        set_selected_attempt(app, None);
        return;
    }

    if let Some(id) = app.selected_attempt_id
        && let Some(idx) = app.attempts.iter().position(|a| a.id == id)
    {
        app.selected_attempt_index = idx;
        return;
    }

    app.selected_attempt_index = app.selected_attempt_index.min(app.attempts.len() - 1);
    set_selected_attempt(app, Some(app.attempts[app.selected_attempt_index].id));
}

fn ensure_exec_selection(app: &mut AppState) {
    let execs = exec_list(&app.exec_store);
    set_selected_exec(app, active_exec_id(&execs));
}

fn sync_tasks_active_column(app: &mut AppState) {
    let Some(task_id) = app.selected_task_id else {
        return;
    };
    let Some(task) = find_task(&app.tasks_store, task_id) else {
        return;
    };
    app.tasks_active_column = match task.status {
        TaskStatus::Cancelled if !app.show_cancelled => TaskStatus::Done,
        other => other,
    };
}

fn prev_board_column(col: TaskStatus) -> TaskStatus {
    match col {
        TaskStatus::Todo => TaskStatus::Todo,
        TaskStatus::InProgress => TaskStatus::Todo,
        TaskStatus::InReview => TaskStatus::InProgress,
        TaskStatus::Done => TaskStatus::InReview,
        TaskStatus::Cancelled => TaskStatus::Done,
    }
}

fn next_board_column(col: TaskStatus) -> TaskStatus {
    match col {
        TaskStatus::Todo => TaskStatus::InProgress,
        TaskStatus::InProgress => TaskStatus::InReview,
        TaskStatus::InReview => TaskStatus::Done,
        TaskStatus::Done => TaskStatus::Done,
        TaskStatus::Cancelled => TaskStatus::Cancelled,
    }
}

struct TasksByStatus {
    todo: Vec<TaskRow>,
    inprogress: Vec<TaskRow>,
    inreview: Vec<TaskRow>,
    done: Vec<TaskRow>,
    cancelled: Vec<TaskRow>,
}

fn tasks_by_status(tasks: &[TaskRow]) -> TasksByStatus {
    let mut out = TasksByStatus {
        todo: vec![],
        inprogress: vec![],
        inreview: vec![],
        done: vec![],
        cancelled: vec![],
    };

    for t in tasks {
        match t.status {
            TaskStatus::Todo => out.todo.push(t.clone()),
            TaskStatus::InProgress => out.inprogress.push(t.clone()),
            TaskStatus::InReview => out.inreview.push(t.clone()),
            TaskStatus::Done => out.done.push(t.clone()),
            TaskStatus::Cancelled => out.cancelled.push(t.clone()),
        }
    }

    let sort = |a: &TaskRow, b: &TaskRow| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.title.cmp(&b.title))
    };
    out.todo.sort_by(sort);
    out.inprogress.sort_by(sort);
    out.inreview.sort_by(sort);
    out.done.sort_by(sort);
    out.cancelled.sort_by(sort);

    out
}

fn tasks_list(store: &serde_json::Value) -> Vec<TaskRow> {
    let tasks_obj = store.get("tasks").and_then(|v| v.as_object());
    let Some(tasks_obj) = tasks_obj else {
        return vec![];
    };

    let mut rows = Vec::with_capacity(tasks_obj.len());
    for (id_str, task_val) in tasks_obj.iter() {
        let Ok(id) = Uuid::parse_str(id_str) else {
            continue;
        };
        let status_str = task_val.get("status").and_then(|v| v.as_str());
        let Some(status) = status_str.and_then(TaskStatus::from_str) else {
            continue;
        };
        let title = task_val
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("(untitled)")
            .to_string();
        let updated_at = task_val
            .get("updated_at")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let has_in_progress_attempt = task_val
            .get("has_in_progress_attempt")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let last_attempt_failed = task_val
            .get("last_attempt_failed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let executor = task_val
            .get("executor")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let description = task_val
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        rows.push(TaskRow {
            id,
            title,
            status,
            updated_at,
            has_in_progress_attempt,
            last_attempt_failed,
            executor,
            description,
        });
    }

    rows
}

fn tasks_filtered_base(app: &AppState) -> Vec<TaskRow> {
    let mut tasks = tasks_list(&app.tasks_store);
    let q = app.task_filter.trim();
    if !q.is_empty() {
        tasks.retain(|t| contains_ci(&t.title, q));
    }
    tasks
}

fn tasks_filtered_table(app: &AppState) -> Vec<TaskRow> {
    tasks_filtered_base(app)
}

fn tasks_filtered_by_status(app: &AppState, status: TaskStatus) -> Vec<TaskRow> {
    let mut tasks = tasks_filtered_base(app);
    tasks.retain(|t| t.status == status);
    tasks.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.title.cmp(&b.title))
    });
    tasks
}

fn find_task(store: &serde_json::Value, task_id: Uuid) -> Option<TaskRow> {
    let task_val = store
        .get("tasks")
        .and_then(|v| v.as_object())?
        .get(&task_id.to_string())?;

    let status_str = task_val.get("status").and_then(|v| v.as_str())?;
    let status = TaskStatus::from_str(status_str)?;
    let title = task_val
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("(untitled)")
        .to_string();

    let updated_at = task_val
        .get("updated_at")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let has_in_progress_attempt = task_val
        .get("has_in_progress_attempt")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let last_attempt_failed = task_val
        .get("last_attempt_failed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let executor = task_val
        .get("executor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let description = task_val
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(TaskRow {
        id: task_id,
        title,
        status,
        updated_at,
        has_in_progress_attempt,
        last_attempt_failed,
        executor,
        description,
    })
}

fn task_index_in(list: &[TaskRow], selected_id: Option<Uuid>) -> Option<usize> {
    let selected_id = selected_id?;
    list.iter().position(|t| t.id == selected_id)
}

fn render_task_line(task: &TaskRow) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![];
    if task.has_in_progress_attempt {
        spans.push(Span::styled("RUN ", Style::default().fg(Color::Green)));
    } else if task.last_attempt_failed {
        spans.push(Span::styled(
            "FAIL",
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::raw(task.title.clone()));
    Line::from(spans)
}

fn select_adjacent_task(app: &mut AppState, delta: i32) {
    let tasks = tasks_filtered_base(app);
    if tasks.is_empty() {
        set_selected_task(app, None);
        return;
    }

    let by_status = tasks_by_status(&tasks);
    let statuses = board_statuses(app);
    if statuses.is_empty() {
        set_selected_task(app, None);
        return;
    }

    let list_for = |status: TaskStatus| -> &[TaskRow] {
        match status {
            TaskStatus::Todo => &by_status.todo,
            TaskStatus::InProgress => &by_status.inprogress,
            TaskStatus::InReview => &by_status.inreview,
            TaskStatus::Done => &by_status.done,
            TaskStatus::Cancelled => &by_status.cancelled,
        }
    };

    // Determine current (status, index) from the selected task if possible; otherwise fall back to
    // the active column + stored index.
    let mut cur_status = app.tasks_active_column;
    if !statuses.contains(&cur_status) {
        cur_status = *statuses.last().unwrap_or(&TaskStatus::Done);
    }
    let mut cur_idx = app.board_index_by_status[cur_status.idx()];
    if let Some(id) = app.selected_task_id {
        for status in &statuses {
            let list = list_for(*status);
            if let Some(pos) = list.iter().position(|t| t.id == id) {
                cur_status = *status;
                cur_idx = pos;
                break;
            }
        }
    }

    // If the current column is empty, jump to the nearest non-empty column in the movement
    // direction (or the first non-empty).
    if list_for(cur_status).is_empty() {
        let order: Box<dyn Iterator<Item = TaskStatus>> = if delta < 0 {
            Box::new(statuses.iter().copied().rev())
        } else {
            Box::new(statuses.iter().copied())
        };
        if let Some(status) = order.into_iter().find(|s| !list_for(*s).is_empty()) {
            cur_status = status;
            cur_idx = if delta < 0 {
                list_for(cur_status).len().saturating_sub(1)
            } else {
                0
            };
        } else {
            set_selected_task(app, None);
            return;
        }
    }

    let cur_status_pos = statuses.iter().position(|s| *s == cur_status).unwrap_or(0);
    let cur_list = list_for(cur_status);

    let (next_status, next_idx) = if delta < 0 {
        if cur_idx > 0 {
            (cur_status, cur_idx - 1)
        } else {
            // Move to the last task of the previous non-empty section (if any).
            let mut s_pos = cur_status_pos;
            let mut found: Option<(TaskStatus, usize)> = None;
            while s_pos > 0 {
                s_pos -= 1;
                let s = statuses[s_pos];
                let list = list_for(s);
                if !list.is_empty() {
                    found = Some((s, list.len() - 1));
                    break;
                }
            }
            found.unwrap_or((cur_status, 0))
        }
    } else {
        if cur_idx + 1 < cur_list.len() {
            (cur_status, cur_idx + 1)
        } else {
            // Move to the first task of the next non-empty section (if any).
            let mut s_pos = cur_status_pos + 1;
            let mut found: Option<(TaskStatus, usize)> = None;
            while s_pos < statuses.len() {
                let s = statuses[s_pos];
                let list = list_for(s);
                if !list.is_empty() {
                    found = Some((s, 0));
                    break;
                }
                s_pos += 1;
            }
            found.unwrap_or((cur_status, cur_idx))
        }
    };

    let next_list = list_for(next_status);
    if next_list.is_empty() {
        set_selected_task(app, None);
        return;
    }
    let next_idx = next_idx.min(next_list.len() - 1);
    app.tasks_active_column = next_status;
    app.board_index_by_status[next_status.idx()] = next_idx;
    set_selected_task(app, Some(next_list[next_idx].id));
}

fn clamp_index(cur: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta < 0 {
        cur.saturating_sub(delta.unsigned_abs() as usize)
    } else {
        (cur + delta as usize).min(len - 1)
    }
}

fn request_move_selected_task(app: &mut AppState, direction: i32) {
    let Some(task_id) = app.selected_task_id else {
        return;
    };
    let Some(task) = find_task(&app.tasks_store, task_id) else {
        return;
    };
    let Some(next) = next_status(task.status, direction) else {
        return;
    };

    let base_url = app.backend_url.clone();
    let net_tx = app.net_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = update_task_status_http(&base_url, task_id, next).await {
            let _ = net_tx
                .send(NetEvent::Error(format!("status update failed: {e}")))
                .await;
        }
    });
}

fn ensure_selected_task_in_active_column(app: &mut AppState) {
    let tasks = tasks_filtered_base(app);
    if tasks.is_empty() {
        set_selected_task(app, None);
        return;
    }

    let by_status = tasks_by_status(&tasks);
    let list: &[TaskRow] = match app.tasks_active_column {
        TaskStatus::Todo => &by_status.todo,
        TaskStatus::InProgress => &by_status.inprogress,
        TaskStatus::InReview => &by_status.inreview,
        TaskStatus::Done => &by_status.done,
        TaskStatus::Cancelled => &by_status.cancelled,
    };
    if list.is_empty() {
        set_selected_task(app, None);
        return;
    }

    if let Some(selected_id) = app.selected_task_id
        && let Some(idx) = list.iter().position(|t| t.id == selected_id)
    {
        app.board_index_by_status[app.tasks_active_column.idx()] = idx;
        return;
    }

    let idx = app.board_index_by_status[app.tasks_active_column.idx()].min(list.len() - 1);
    set_selected_task(app, Some(list[idx].id));
}

fn next_status(status: TaskStatus, direction: i32) -> Option<TaskStatus> {
    let chain = [
        TaskStatus::Todo,
        TaskStatus::InProgress,
        TaskStatus::InReview,
        TaskStatus::Done,
    ];
    let idx = chain.iter().position(|s| *s == status)?;
    if direction < 0 {
        idx.checked_sub(1).map(|i| chain[i])
    } else {
        chain.get(idx + 1).copied()
    }
}

async fn update_task_status_http(
    base_url: &str,
    task_id: Uuid,
    status: TaskStatus,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!("{}/api/tasks/{}", base_url.trim_end_matches('/'), task_id);
    let body = serde_json::json!({ "status": status.as_api_str() });

    let resp = client.put(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected status update");
    }
    Ok(())
}

async fn stop_exec_http(base_url: &str, exec_id: Uuid) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/execution-processes/{}/stop",
        base_url.trim_end_matches('/'),
        exec_id
    );

    let resp = client.post(url).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected stop request");
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct SessionDto {
    id: Uuid,
}

async fn latest_session_id_http(
    base_url: &str,
    workspace_id: Uuid,
) -> anyhow::Result<Option<Uuid>> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/sessions?workspace_id={workspace_id}",
        base_url.trim_end_matches('/')
    );

    let resp = client.get(url).send().await?;
    let api = resp.json::<ApiResponse<Vec<SessionDto>>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected sessions request");
    }
    Ok(api
        .into_data()
        .unwrap_or_default()
        .into_iter()
        .next()
        .map(|s| s.id))
}

async fn queue_follow_up_http(
    base_url: &str,
    session_id: Uuid,
    message: &str,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/sessions/{session_id}/queue",
        base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({ "message": message, "variant": null });

    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected queue request");
    }
    Ok(())
}

async fn follow_up_http(base_url: &str, session_id: Uuid, prompt: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/sessions/{session_id}/follow-up",
        base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "prompt": prompt,
        "variant": null,
        "retry_process_id": null,
        "force_when_dirty": null,
        "perform_git_reset": null,
    });

    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponse<serde_json::Value>>().await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected follow-up request");
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct ApiResponseWire<T, E = serde_json::Value> {
    success: bool,
    data: Option<T>,
    error_data: Option<E>,
    message: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct RepoIdRequest {
    repo_id: Uuid,
}

#[derive(Debug, serde::Serialize)]
struct RebaseTaskAttemptRequest {
    repo_id: Uuid,
    old_base_branch: Option<String>,
    new_base_branch: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum GitOperationErrorWire {
    MergeConflicts { message: String, op: ConflictOp },
    RebaseInProgress,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum PushErrorWire {
    ForcePushRequired,
}

#[derive(Debug, serde::Serialize)]
struct CreateGitHubPrRequest {
    title: String,
    body: Option<String>,
    target_branch: Option<String>,
    draft: Option<bool>,
    repo_id: Uuid,
    #[serde(default)]
    auto_generate_description: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum CreatePrErrorWire {
    GithubCliNotInstalled,
    GithubCliNotLoggedIn,
    GitCliNotLoggedIn,
    GitCliNotInstalled,
    TargetBranchNotFound { branch: String },
}

#[derive(Debug, serde::Deserialize)]
struct AttachPrResponse {
    pr_attached: bool,
    pr_url: Option<String>,
    #[allow(dead_code)]
    pr_number: Option<i64>,
    #[allow(dead_code)]
    pr_status: Option<MergeStatus>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum GetPrCommentsErrorWire {
    NoPrAttached,
    GithubCliNotInstalled,
    GithubCliNotLoggedIn,
}

#[derive(Debug, serde::Deserialize)]
struct PrCommentsResponse {
    comments: Vec<serde_json::Value>,
}

#[derive(Debug, serde::Serialize)]
struct OpenEditorRequest {
    editor_type: Option<String>,
    file_path: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct OpenEditorResponse {
    url: Option<String>,
}

async fn branch_status_http(
    base_url: &str,
    attempt_id: Uuid,
) -> anyhow::Result<Vec<RepoBranchStatus>> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/branch-status",
        base_url.trim_end_matches('/')
    );
    let resp = client.get(url).send().await?;
    let api = resp
        .json::<ApiResponseWire<Vec<RepoBranchStatus>>>()
        .await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected branch status request")
        );
    }
    Ok(api.data.unwrap_or_default())
}

async fn rebase_task_attempt_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
    old_base_branch: Option<String>,
    new_base_branch: Option<String>,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/rebase",
        base_url.trim_end_matches('/')
    );
    let body = RebaseTaskAttemptRequest {
        repo_id,
        old_base_branch,
        new_base_branch,
    };

    let resp = client.post(url).json(&body).send().await?;
    let api = resp
        .json::<ApiResponseWire<serde_json::Value, GitOperationErrorWire>>()
        .await?;
    if api.success {
        return Ok(());
    }

    if let Some(msg) = api.message {
        anyhow::bail!("{msg}");
    }
    if let Some(err) = api.error_data {
        match err {
            GitOperationErrorWire::MergeConflicts { message, .. } => anyhow::bail!("{message}"),
            GitOperationErrorWire::RebaseInProgress => {
                anyhow::bail!("rebase already in progress")
            }
        }
    }
    anyhow::bail!("backend rejected rebase request");
}

async fn abort_conflicts_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/conflicts/abort",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = resp.json::<ApiResponseWire<serde_json::Value>>().await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected abort request")
        );
    }
    Ok(())
}

async fn merge_task_attempt_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/merge",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = resp.json::<ApiResponseWire<serde_json::Value>>().await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected merge request")
        );
    }
    Ok(())
}

async fn push_task_attempt_branch_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/push",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = resp
        .json::<ApiResponseWire<serde_json::Value, PushErrorWire>>()
        .await?;
    if api.success {
        return Ok(());
    }
    if let Some(PushErrorWire::ForcePushRequired) = api.error_data {
        anyhow::bail!("push rejected (use /push --force)");
    }
    anyhow::bail!(
        "{}",
        api.message
            .as_deref()
            .unwrap_or("backend rejected push request")
    );
}

async fn force_push_task_attempt_branch_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/push/force",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = resp
        .json::<ApiResponseWire<serde_json::Value, PushErrorWire>>()
        .await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected force-push request")
        );
    }
    Ok(())
}

async fn create_pr_http(
    base_url: &str,
    attempt_id: Uuid,
    request: CreateGitHubPrRequest,
) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/pr",
        base_url.trim_end_matches('/')
    );
    let resp = client.post(url).json(&request).send().await?;
    let api = resp
        .json::<ApiResponseWire<String, CreatePrErrorWire>>()
        .await?;
    if api.success {
        return Ok(api.data.unwrap_or_default());
    }
    if let Some(msg) = api.message {
        anyhow::bail!("{msg}");
    }
    if let Some(err) = api.error_data {
        let msg = match err {
            CreatePrErrorWire::GithubCliNotInstalled => {
                "GitHub CLI (gh) not installed on the server"
            }
            CreatePrErrorWire::GithubCliNotLoggedIn => "GitHub CLI (gh) not logged in",
            CreatePrErrorWire::GitCliNotLoggedIn => "git not authenticated (CLI auth failed)",
            CreatePrErrorWire::GitCliNotInstalled => "git not available on the server",
            CreatePrErrorWire::TargetBranchNotFound { branch } => {
                return Err(anyhow::anyhow!("target branch not found: {branch}"));
            }
        };
        anyhow::bail!("{msg}");
    }
    anyhow::bail!("backend rejected PR create request");
}

async fn attach_pr_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<AttachPrResponse> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/pr/attach",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&RepoIdRequest { repo_id })
        .send()
        .await?;
    let api = resp.json::<ApiResponseWire<AttachPrResponse>>().await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected PR attach request")
        );
    }
    Ok(api.data.unwrap_or(AttachPrResponse {
        pr_attached: false,
        pr_url: None,
        pr_number: None,
        pr_status: None,
    }))
}

async fn get_pr_comments_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
) -> anyhow::Result<usize> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/pr/comments?repo_id={repo_id}",
        base_url.trim_end_matches('/')
    );
    let resp = client.get(url).send().await?;
    let api = resp
        .json::<ApiResponseWire<PrCommentsResponse, GetPrCommentsErrorWire>>()
        .await?;
    if api.success {
        return Ok(api.data.map(|d| d.comments.len()).unwrap_or(0));
    }
    if let Some(msg) = api.message {
        anyhow::bail!("{msg}");
    }
    if let Some(err) = api.error_data {
        let msg = match err {
            GetPrCommentsErrorWire::NoPrAttached => "no PR attached",
            GetPrCommentsErrorWire::GithubCliNotInstalled => {
                "GitHub CLI (gh) not installed on the server"
            }
            GetPrCommentsErrorWire::GithubCliNotLoggedIn => "GitHub CLI (gh) not logged in",
        };
        anyhow::bail!("{msg}");
    }
    anyhow::bail!("backend rejected PR comments request");
}

async fn open_editor_http(
    base_url: &str,
    attempt_id: Uuid,
    file_path: Option<String>,
) -> anyhow::Result<Option<String>> {
    let client = reqwest::Client::builder()
        .build()
        .context("build reqwest client")?;

    let url = format!(
        "{}/api/task-attempts/{attempt_id}/open-editor",
        base_url.trim_end_matches('/')
    );
    let body = OpenEditorRequest {
        editor_type: None,
        file_path,
    };
    let resp = client.post(url).json(&body).send().await?;
    let api = resp.json::<ApiResponseWire<OpenEditorResponse>>().await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected open-editor request")
        );
    }
    Ok(api.data.and_then(|d| d.url))
}
