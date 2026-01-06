use std::{collections::HashMap, time::Instant};

use ratatui::text::Line;
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use super::types::{
    AttemptRow, BranchPickerState, ConfirmState, CreateTaskState, DiffFocus, DiffTheme,
    ExecutorProfileSelection, FocusPane, GitOpState, InputState, JobKey, LogMode, LogRenderMode,
    LogViewMode, PendingExecHook, RepoBranchStatus, TaskStatus, TextFieldState, ToastState,
    TuiPrefs,
};
use crate::{
    events::{NetEvent, StreamStatus},
    logs::{ExecLogBuffer, LogSelection},
};

pub(crate) struct BoardState {
    pub(crate) project_filter: String,

    pub(crate) projects_status: StreamStatus,
    pub(crate) projects_store: serde_json::Value,
    pub(crate) projects_loaded_once: bool,
    pub(crate) selected_project_id: Option<Uuid>,
    pub(crate) selected_project_index: usize,

    pub(crate) task_filter: String,
    pub(crate) show_cancelled: bool,

    pub(crate) tasks_status: StreamStatus,
    pub(crate) tasks_store: serde_json::Value,
    pub(crate) selected_task_id: Option<Uuid>,
    pub(crate) pending_select_task_id: Option<Uuid>,
    pub(crate) tasks_active_column: TaskStatus,
    pub(crate) board_index_by_status: [usize; 5],

    pub(crate) attempts: Vec<AttemptRow>,
    pub(crate) selected_attempt_id: Option<Uuid>,
    pub(crate) selected_attempt_index: usize,
}

pub(crate) struct ExecState {
    pub(crate) exec_status: StreamStatus,
    pub(crate) exec_store: serde_json::Value,
    pub(crate) selected_exec_id: Option<Uuid>,

    pub(crate) log_status: StreamStatus,
    pub(crate) log_lines: Vec<Line<'static>>,
    pub(crate) log_line_targets: Vec<Option<LogSelection>>,
    pub(crate) log_selected: Option<LogSelection>,
    pub(crate) log_mode: LogMode,
    pub(crate) log_render_mode: LogRenderMode,
    pub(crate) log_view_mode: LogViewMode,
    pub(crate) log_render_width: u16,
    pub(crate) log_target_render_width: u16,
    pub(crate) log_prewarm_job_width: Option<u16>,
    pub(crate) log_prewarm_gen: u64,
    pub(crate) log_autoscroll: bool,
    pub(crate) log_scroll_offset: usize,

    pub(crate) log_buffers: HashMap<Uuid, ExecLogBuffer>,
    pub(crate) log_exec_order: Vec<Uuid>,
    pub(crate) log_view_dirty: bool,

    pub(crate) pending_user_log: Option<String>,
    pub(crate) pending_user_log_prev_exec_id: Option<Uuid>,
    pub(crate) pending_user_log_wait_new_exec: bool,

    pub(crate) pending_branch_refresh: Option<PendingExecHook>,
}

pub(crate) struct DiffState {
    pub(crate) diff_status: StreamStatus,
    pub(crate) diff_store: serde_json::Value,
    pub(crate) diff_stats_only: bool,
    pub(crate) selected_diff_index: usize,
    pub(crate) diff_scroll_offset: usize,
    pub(crate) diff_preview_cache_key: Option<String>,
    pub(crate) diff_preview_cache_hash: u64,
    pub(crate) diff_preview_cache_width: u16,
    pub(crate) diff_preview_lines: Vec<Line<'static>>,
    pub(crate) diff_theme: DiffTheme,
    pub(crate) diff_wrap: bool,
    pub(crate) diff_preview_pending: bool,
    pub(crate) diff_preview_next_refresh_at: Option<Instant>,
    pub(crate) diff_preview_gen: u64,

    pub(crate) repo_statuses: Vec<RepoBranchStatus>,
    pub(crate) selected_repo_index: usize,

    pub(crate) git_ops: HashMap<Uuid, GitOpState>,
    pub(crate) git_op_global: Option<GitOpState>,
}

pub(crate) struct UiState {
    pub(crate) focus: FocusPane,
    pub(crate) diff_focus: DiffFocus,
    pub(crate) show_help: bool,

    pub(crate) input: Option<InputState>,
    pub(crate) confirm: Option<ConfirmState>,
    pub(crate) create_task: Option<CreateTaskState>,
    pub(crate) project_setup: Option<super::types::ProjectSetupState>,
    pub(crate) project_setup_dismissed: bool,
    pub(crate) branch_picker: Option<BranchPickerState>,
    pub(crate) launch_repo_path: Option<String>,
    pub(crate) launch_suggested_project_name: String,
    pub(crate) launch_dir_explicit: bool,
    pub(crate) launch_match_done: bool,

    pub(crate) composer_active: bool,
    pub(crate) composer: TextFieldState,
    pub(crate) composer_suggest_index: usize,
    pub(crate) refresh_branch_status_after_send: bool,

    pub(crate) last_error: Option<String>,
    pub(crate) last_notice: Option<String>,

    pub(crate) toast: Option<ToastState>,

    pub(crate) available_executors: Vec<String>,
    pub(crate) selected_executor_profile: Option<ExecutorProfileSelection>,
    pub(crate) executor_profiles: serde_json::Value,
}

pub(crate) struct AppState {
    pub(crate) backend_url: String,
    pub(crate) info_summary: String,
    pub(crate) info_ok: bool,

    pub(crate) prefs: TuiPrefs,

    pub(crate) ui: UiState,
    pub(crate) board: BoardState,
    pub(crate) exec: ExecState,
    pub(crate) diff: DiffState,

    pub(crate) jobs: HashMap<JobKey, tokio::task::JoinHandle<()>>,

    pub(crate) net_tx: mpsc::Sender<NetEvent>,
    pub(crate) project_sel_tx: watch::Sender<Option<Uuid>>,
    pub(crate) attempt_sel_tx: watch::Sender<Option<Uuid>>,
    pub(crate) exec_sel_tx: watch::Sender<Option<Uuid>>,
    pub(crate) log_mode_tx: watch::Sender<LogMode>,
    pub(crate) diff_stats_tx: watch::Sender<bool>,
    pub(crate) diff_reconnect_tx: watch::Sender<u64>,
    pub(crate) reconnect_tx: watch::Sender<u64>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        backend_url: String,
        net_tx: mpsc::Sender<NetEvent>,
        project_sel_tx: watch::Sender<Option<Uuid>>,
        attempt_sel_tx: watch::Sender<Option<Uuid>>,
        exec_sel_tx: watch::Sender<Option<Uuid>>,
        log_mode_tx: watch::Sender<LogMode>,
        diff_stats_tx: watch::Sender<bool>,
        diff_reconnect_tx: watch::Sender<u64>,
        reconnect_tx: watch::Sender<u64>,
        prefs: TuiPrefs,
    ) -> Self {
        let (launch_repo_path, launch_suggested_project_name) = {
            let launch_dir = std::env::var("VIBE_TUI_PROJECT_DIR")
                .ok()
                .or_else(|| std::env::var("VIBE_PROJECT_DIR").ok())
                .filter(|s| !s.trim().is_empty())
                .map(std::path::PathBuf::from);
            let cwd = launch_dir.or_else(|| std::env::current_dir().ok());
            let mut repo: Option<std::path::PathBuf> = None;
            if let Some(mut cur) = cwd.clone() {
                loop {
                    if cur.join(".git").exists() {
                        repo = Some(cur.clone());
                        break;
                    }
                    if !cur.pop() {
                        break;
                    }
                }
            }
            let suggested = repo
                .as_ref()
                .or(cwd.as_ref())
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| "project".to_string());
            (
                repo.or(cwd).map(|p| p.to_string_lossy().to_string()),
                suggested,
            )
        };
        let launch_dir_explicit = std::env::var("VIBE_TUI_PROJECT_DIR")
            .ok()
            .or_else(|| std::env::var("VIBE_PROJECT_DIR").ok())
            .is_some();

        let state = Self {
            backend_url,
            info_summary: "loading /api/info…".to_string(),
            info_ok: false,

            prefs: prefs.clone(),

            ui: UiState {
                focus: FocusPane::Board,
                diff_focus: DiffFocus::Files,
                show_help: false,

                input: None,
                confirm: None,
                create_task: None,
                project_setup: None,
                project_setup_dismissed: false,
                branch_picker: None,
                launch_repo_path,
                launch_suggested_project_name,
                launch_dir_explicit,
                launch_match_done: false,

                composer_active: false,
                composer: Default::default(),
                composer_suggest_index: 0,
                refresh_branch_status_after_send: false,

                last_error: None,
                last_notice: None,

                toast: None,

                available_executors: vec![],
                selected_executor_profile: None,
                executor_profiles: serde_json::json!({}),
            },

            board: BoardState {
                project_filter: String::new(),

                projects_status: StreamStatus::Disconnected,
                projects_store: serde_json::json!({ "projects": {} }),
                projects_loaded_once: false,
                selected_project_id: prefs.selected_project_id,
                selected_project_index: 0,

                task_filter: String::new(),
                show_cancelled: prefs.show_cancelled,

                tasks_status: StreamStatus::Disconnected,
                tasks_store: serde_json::json!({ "tasks": {} }),
                selected_task_id: None,
                pending_select_task_id: None,
                tasks_active_column: TaskStatus::Todo,
                board_index_by_status: [0; 5],

                attempts: vec![],
                selected_attempt_id: None,
                selected_attempt_index: 0,
            },

            exec: ExecState {
                exec_status: StreamStatus::Disconnected,
                exec_store: serde_json::json!({ "execution_processes": {} }),
                selected_exec_id: None,

                log_status: StreamStatus::Disconnected,
                log_lines: vec![],
                log_mode: prefs.log_mode,
                log_render_mode: prefs.log_render_mode,
                log_view_mode: prefs.log_view_mode,
                log_render_width: 0,
                log_target_render_width: 0,
                log_prewarm_job_width: None,
                log_prewarm_gen: 0,
                log_autoscroll: true,
                log_scroll_offset: 0,
                log_line_targets: vec![],
                log_selected: None,

                log_buffers: HashMap::new(),
                log_exec_order: vec![],
                log_view_dirty: true,

                pending_user_log: None,
                pending_user_log_prev_exec_id: None,
                pending_user_log_wait_new_exec: false,

                pending_branch_refresh: None,
            },

            diff: DiffState {
                diff_status: StreamStatus::Disconnected,
                diff_store: serde_json::json!({ "entries": {} }),
                diff_stats_only: false,
                selected_diff_index: 0,
                diff_scroll_offset: 0,
                diff_preview_cache_key: None,
                diff_preview_cache_hash: 0,
                diff_preview_cache_width: 0,
                diff_preview_lines: vec![Line::from("No diffs")],
                diff_theme: prefs.diff_theme,
                diff_wrap: prefs.diff_wrap,
                diff_preview_pending: false,
                diff_preview_next_refresh_at: None,
                diff_preview_gen: 0,

                repo_statuses: vec![],
                selected_repo_index: 0,

                git_ops: HashMap::new(),
                git_op_global: None,
            },

            jobs: HashMap::new(),

            net_tx,
            project_sel_tx,
            attempt_sel_tx,
            exec_sel_tx,
            log_mode_tx,
            diff_stats_tx,
            diff_reconnect_tx,
            reconnect_tx,
        };

        let _ = state.project_sel_tx.send(state.board.selected_project_id);
        let _ = state.diff_stats_tx.send(state.diff.diff_stats_only);

        state
    }
}
