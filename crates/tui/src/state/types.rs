use std::time::Instant;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::events::GitOpKind;

#[derive(Debug, Clone)]
pub(crate) struct GitOpState {
    pub(crate) kind: GitOpKind,
    pub(crate) started_at: Instant,
    pub(crate) finished_at: Option<Instant>,
    pub(crate) ok: Option<bool>,
}

#[derive(Debug, Clone)]
pub(crate) struct ToastState {
    pub(crate) message: String,
    pub(crate) color: Color,
    pub(crate) expires_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusPane {
    Board,
    Execution,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LogMode {
    Normalized,
    Raw,
}

impl LogMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Normalized => "normalized",
            Self::Raw => "raw",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LogRenderMode {
    Plain,
    Markdown,
}

impl LogRenderMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Markdown => "md",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiffFocus {
    Files,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputMode {
    SearchTasks,
}

#[derive(Debug, Clone)]
pub(crate) struct InputState {
    pub(crate) mode: InputMode,
    pub(crate) field: TextFieldState,
    pub(crate) original: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ConfirmAction {
    StopExec { exec_id: Uuid },
}

#[derive(Debug, Clone)]
pub(crate) struct ConfirmState {
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) action: ConfirmAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreateTaskFocus {
    Title,
    Description,
    Status,
    Buttons,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TextFieldState {
    pub(crate) buffer: String,
    pub(crate) cursor: usize, // byte index
    pub(crate) goal_col: Option<usize>,
    pub(crate) scroll_x: u16, // columns
    pub(crate) scroll_y: u16, // lines
    undo: Vec<TextFieldSnapshot>,
    redo: Vec<TextFieldSnapshot>,
}

#[derive(Debug, Clone)]
struct TextFieldSnapshot {
    buffer: String,
    cursor: usize,
    goal_col: Option<usize>,
    scroll_x: u16,
    scroll_y: u16,
}

impl TextFieldState {
    fn snapshot(&self) -> TextFieldSnapshot {
        TextFieldSnapshot {
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            goal_col: self.goal_col,
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
        }
    }

    fn restore(&mut self, snap: TextFieldSnapshot) {
        self.buffer = snap.buffer;
        self.cursor = snap.cursor;
        self.goal_col = snap.goal_col;
        self.scroll_x = snap.scroll_x;
        self.scroll_y = snap.scroll_y;
        self.clamp_cursor();
    }

    fn push_undo(&mut self) {
        const MAX_UNDO: usize = 200;
        self.undo.push(self.snapshot());
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub(crate) fn undo(&mut self) -> bool {
        let Some(snap) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(snap);
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some(snap) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(snap);
        true
    }

    pub(crate) fn clamp_cursor(&mut self) {
        self.cursor = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
    }

    pub(crate) fn set_end(&mut self) {
        self.cursor = self.buffer.len();
        self.goal_col = None;
    }

    pub(crate) fn clear(&mut self) {
        if !self.buffer.is_empty() {
            self.push_undo();
        }
        self.buffer.clear();
        self.cursor = 0;
        self.goal_col = None;
        self.scroll_x = 0;
        self.scroll_y = 0;
    }

    pub(crate) fn move_left(&mut self) {
        self.cursor = crate::text::edit::prev_cursor(&self.buffer, self.cursor);
        self.goal_col = None;
    }

    pub(crate) fn move_right(&mut self) {
        self.cursor = crate::text::edit::next_cursor(&self.buffer, self.cursor);
        self.goal_col = None;
    }

    pub(crate) fn move_word_left(&mut self) {
        self.cursor = crate::text::edit::prev_word_cursor(&self.buffer, self.cursor);
        self.goal_col = None;
    }

    pub(crate) fn move_word_right(&mut self) {
        self.cursor = crate::text::edit::next_word_cursor(&self.buffer, self.cursor);
        self.goal_col = None;
    }

    pub(crate) fn move_home(&mut self, multiline: bool) {
        if !multiline {
            self.cursor = 0;
            self.goal_col = None;
            return;
        }
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let ranges = crate::text::edit::line_ranges(&self.buffer);
        let line = crate::text::edit::cursor_line_index(&ranges, cur);
        self.cursor = ranges.get(line).map(|(s, _)| *s).unwrap_or(0);
        self.goal_col = None;
    }

    pub(crate) fn move_end(&mut self, multiline: bool) {
        if !multiline {
            self.cursor = self.buffer.len();
            self.goal_col = None;
            return;
        }
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let ranges = crate::text::edit::line_ranges(&self.buffer);
        let line = crate::text::edit::cursor_line_index(&ranges, cur);
        self.cursor = ranges
            .get(line)
            .map(|(_, e)| *e)
            .unwrap_or(self.buffer.len());
        self.goal_col = None;
    }

    pub(crate) fn move_up(&mut self) {
        let (next, goal) = crate::text::edit::move_cursor_vertically(
            &self.buffer,
            self.cursor,
            -1,
            self.goal_col,
        );
        self.cursor = next;
        self.goal_col = goal;
    }

    pub(crate) fn move_down(&mut self) {
        let (next, goal) = crate::text::edit::move_cursor_vertically(
            &self.buffer,
            self.cursor,
            1,
            self.goal_col,
        );
        self.cursor = next;
        self.goal_col = goal;
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        self.push_undo();
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        self.buffer.insert(cur, ch);
        self.cursor = cur + ch.len_utf8();
        self.goal_col = None;
    }

    pub(crate) fn backspace(&mut self) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let prev = crate::text::edit::prev_cursor(&self.buffer, cur);
        if prev < cur {
            self.push_undo();
            self.buffer.replace_range(prev..cur, "");
            self.cursor = prev;
        } else {
            self.cursor = 0;
        }
        self.goal_col = None;
    }

    pub(crate) fn backspace_word(&mut self) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let prev = crate::text::edit::prev_word_cursor(&self.buffer, cur);
        if prev < cur {
            self.push_undo();
            self.buffer.replace_range(prev..cur, "");
            self.cursor = prev;
        } else {
            self.cursor = 0;
        }
        self.goal_col = None;
    }

    pub(crate) fn delete(&mut self) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let next = crate::text::edit::next_cursor(&self.buffer, cur);
        if cur < next {
            self.push_undo();
            self.buffer.replace_range(cur..next, "");
            self.cursor = cur;
        } else {
            self.cursor = self.buffer.len();
        }
        self.goal_col = None;
    }

    pub(crate) fn delete_word(&mut self) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let next = crate::text::edit::next_word_cursor(&self.buffer, cur);
        if cur < next {
            self.push_undo();
            self.buffer.replace_range(cur..next, "");
            self.cursor = cur;
        } else {
            self.cursor = self.buffer.len();
        }
        self.goal_col = None;
    }

    pub(crate) fn cursor_line_col(&self) -> (usize, usize) {
        let cur = crate::text::edit::clamp_cursor_to_boundary(&self.buffer, self.cursor);
        let ranges = crate::text::edit::line_ranges(&self.buffer);
        let line = crate::text::edit::cursor_line_index(&ranges, cur);
        let (line_start, line_end) = ranges.get(line).copied().unwrap_or((0, 0));
        let col = crate::text::edit::cursor_col_in_line(&self.buffer, line_start, cur.min(line_end));
        (line, col)
    }

    pub(crate) fn ensured_scroll(&self, inner_w: usize, inner_h: usize) -> (u16, u16) {
        let (line, col) = self.cursor_line_col();

        let inner_h = inner_h.max(1) as i64;
        let inner_w = inner_w.max(1) as i64;
        let cy = line as i64;
        let cx = col as i64;
        let sy = self.scroll_y as i64;
        let sx = self.scroll_x as i64;

        let mut new_sy = sy;
        if cy < sy {
            new_sy = cy;
        } else if cy >= sy + inner_h {
            new_sy = cy - inner_h + 1;
        }

        let mut new_sx = sx;
        if cx < sx {
            new_sx = cx;
        } else if cx >= sx + inner_w {
            new_sx = cx - inner_w + 1;
        }

        (new_sy.max(0) as u16, new_sx.max(0) as u16)
    }

    pub(crate) fn ensure_cursor_visible(&mut self, inner_w: usize, inner_h: usize) {
        let (y, x) = self.ensured_scroll(inner_w, inner_h);
        self.scroll_y = y;
        self.scroll_x = x;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CreateTaskState {
    pub(crate) title: TextFieldState,
    pub(crate) description: TextFieldState,
    pub(crate) status: TaskStatus,
    pub(crate) focus: CreateTaskFocus,
    pub(crate) selected_button: usize, // 0 = create, 1 = cancel
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LogViewMode {
    Timeline,
    Single,
}

impl Default for LogViewMode {
    fn default() -> Self {
        Self::Timeline
    }
}

impl LogViewMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Timeline => "timeline",
            Self::Single => "run",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct TuiPrefs {
    pub(crate) selected_project_id: Option<Uuid>,
    pub(crate) show_cancelled: bool,
    pub(crate) log_mode: LogMode,
    pub(crate) log_render_mode: LogRenderMode,
    pub(crate) log_view_mode: LogViewMode,
    pub(crate) diff_theme: DiffTheme,
    pub(crate) diff_wrap: bool,
}

impl Default for TuiPrefs {
    fn default() -> Self {
        Self {
            selected_project_id: None,
            show_cancelled: false,
            log_mode: LogMode::Normalized,
            log_render_mode: LogRenderMode::Markdown,
            log_view_mode: LogViewMode::Timeline,
            diff_theme: DiffTheme::default(),
            diff_wrap: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiffTheme {
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
    pub(crate) fn is_light(self) -> bool {
        matches!(
            self,
            Self::Base16OceanLight | Self::InspiredGithub | Self::SolarizedLight
        )
    }

    pub(crate) fn syntect_key(self) -> &'static str {
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

    pub(crate) fn label(self) -> &'static str {
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

    pub(crate) fn cycle_next(self) -> Self {
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
pub(crate) enum TaskStatus {
    Todo,
    InProgress,
    InReview,
    Done,
    Cancelled,
}

impl TaskStatus {
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "todo" => Some(Self::Todo),
            "inprogress" => Some(Self::InProgress),
            "inreview" => Some(Self::InReview),
            "done" => Some(Self::Done),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub(crate) fn as_api_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "inprogress",
            Self::InReview => "inreview",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Todo => "Todo",
            Self::InProgress => "In Progress",
            Self::InReview => "In Review",
            Self::Done => "Done",
            Self::Cancelled => "Cancelled",
        }
    }

    pub(crate) fn idx(self) -> usize {
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
pub(crate) struct TaskRow {
    pub(crate) id: Uuid,
    pub(crate) title: String,
    pub(crate) status: TaskStatus,
    pub(crate) updated_at: Option<String>,
    pub(crate) has_in_progress_attempt: bool,
    pub(crate) last_attempt_failed: bool,
    pub(crate) executor: Option<String>,
    #[allow(dead_code)]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AttemptRow {
    pub(crate) id: Uuid,
    pub(crate) branch: String,
    pub(crate) created_at: Option<String>,
    pub(crate) updated_at: Option<String>,
    pub(crate) setup_completed_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExecRow {
    pub(crate) id: Uuid,
    pub(crate) session_id: Option<Uuid>,
    pub(crate) run_reason: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) created_at: Option<String>,
    pub(crate) dropped: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConflictOp {
    Rebase,
    Merge,
    CherryPick,
    Revert,
}

pub(crate) fn display_conflict_op_label(op: Option<ConflictOp>) -> &'static str {
    match op {
        Some(ConflictOp::Merge) => "Merge",
        Some(ConflictOp::CherryPick) => "Cherry-pick",
        Some(ConflictOp::Revert) => "Revert",
        Some(ConflictOp::Rebase) | None => "Rebase",
    }
}

pub(crate) fn format_conflict_header(
    op: Option<ConflictOp>,
    source_branch: &str,
    base_branch: &str,
    repo_name: Option<&str>,
) -> String {
    let repo_context = repo_name
        .filter(|s| !s.trim().is_empty())
        .map(|r| format!(" in repository '{r}'"))
        .unwrap_or_default();
    match op {
        Some(ConflictOp::Merge) => {
            format!("Merge conflicts while merging into '{source_branch}'{repo_context}.")
        }
        Some(ConflictOp::CherryPick) => {
            format!("Cherry-pick conflicts on '{source_branch}'{repo_context}.")
        }
        Some(ConflictOp::Revert) => format!("Revert conflicts on '{source_branch}'{repo_context}."),
        Some(ConflictOp::Rebase) | None => format!(
            "Rebase conflicts while rebasing '{source_branch}' onto '{base_branch}'{repo_context}."
        ),
    }
}

pub(crate) fn build_resolve_conflicts_instructions(
    source_branch: Option<&str>,
    base_branch: Option<&str>,
    conflicted_files: &[String],
    op: Option<ConflictOp>,
    repo_name: Option<&str>,
) -> String {
    let source = source_branch
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("current attempt branch");
    let base = base_branch
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("base branch");

    let files_list: Vec<&String> = conflicted_files.iter().take(12).collect();
    let files_block = if files_list.is_empty() {
        String::new()
    } else {
        let mut out = String::from("\n\nFiles with conflicts:\n");
        for f in files_list {
            out.push_str("- ");
            out.push_str(f);
            out.push('\n');
        }
        out.pop(); // trailing '\n'
        out
    };

    let op_title = display_conflict_op_label(op);
    let header = format_conflict_header(op, source, base, repo_name);

    format!(
        "{header}{files_block}\n\nPlease resolve each file carefully. When continuing, ensure the {} does not hang (set `GIT_EDITOR=true` or use a non-interactive editor).",
        op_title.to_ascii_lowercase()
    )
}

#[cfg(test)]
mod conflict_instruction_tests {
    use super::*;

    #[test]
    fn builds_rebase_instructions_with_files() {
        let out = build_resolve_conflicts_instructions(
            Some("feat/x"),
            Some("main"),
            &vec!["a.txt".to_string(), "b.txt".to_string()],
            Some(ConflictOp::Rebase),
            Some("repo1"),
        );

        assert!(out.contains("Rebase conflicts while rebasing 'feat/x' onto 'main' in repository 'repo1'."));
        assert!(out.contains("Files with conflicts:\n- a.txt\n- b.txt"));
        assert!(out.contains("ensure the rebase does not hang"));
    }

    #[test]
    fn limits_conflicted_files_to_12() {
        let files: Vec<String> = (1..=20).map(|i| format!("file{i}.txt")).collect();
        let out = build_resolve_conflicts_instructions(
            Some("feat/x"),
            Some("main"),
            &files,
            Some(ConflictOp::Merge),
            Some("repo1"),
        );
        assert!(out.contains("- file12.txt"));
        assert!(!out.contains("- file13.txt"));
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MergeStatus {
    Open,
    Merged,
    Closed,
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PullRequestInfo {
    pub(crate) number: i64,
    #[allow(dead_code)]
    pub(crate) url: String,
    pub(crate) status: MergeStatus,
    #[allow(dead_code)]
    pub(crate) merged_at: Option<String>,
    #[allow(dead_code)]
    pub(crate) merge_commit_sha: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PrMerge {
    pub(crate) pr_info: PullRequestInfo,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Merge {
    #[allow(dead_code)]
    Direct(serde_json::Value),
    Pr(PrMerge),
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BranchStatus {
    pub(crate) commits_behind: Option<usize>,
    pub(crate) commits_ahead: Option<usize>,
    pub(crate) has_uncommitted_changes: Option<bool>,
    pub(crate) uncommitted_count: Option<usize>,
    pub(crate) untracked_count: Option<usize>,
    pub(crate) target_branch_name: String,
    pub(crate) remote_commits_behind: Option<usize>,
    pub(crate) remote_commits_ahead: Option<usize>,
    pub(crate) merges: Vec<Merge>,
    pub(crate) is_rebase_in_progress: bool,
    pub(crate) conflict_op: Option<ConflictOp>,
    pub(crate) conflicted_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RepoBranchStatus {
    pub(crate) repo_id: Uuid,
    pub(crate) repo_name: String,
    #[serde(flatten)]
    pub(crate) status: BranchStatus,
}
