use std::time::{Duration, Instant};

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::events::GitOpKind;

#[derive(Debug, Clone)]
pub(crate) struct LoadingState {
    pub(crate) started_at: Option<Instant>,
    pub(crate) delay: Duration,
    pub(crate) pending: bool,
    pub(crate) visible: bool,
    pub(crate) placeholder_pending: bool,
}

impl LoadingState {
    pub(crate) fn start(&mut self, now: Instant, delay: Duration, placeholder_pending: bool) {
        self.placeholder_pending = placeholder_pending;
        self.started_at = Some(now);
        self.delay = delay;
        self.pending = true;
        self.visible = false;
    }

    pub(crate) fn stop(&mut self) {
        self.placeholder_pending = false;
        self.started_at = None;
        self.pending = false;
        self.visible = false;
    }

    pub(crate) fn tick(&mut self, now: Instant, is_running: bool) -> bool {
        if !self.pending || !is_running {
            return false;
        }
        let Some(started) = self.started_at else {
            return false;
        };
        if now.saturating_duration_since(started) < self.delay {
            return false;
        }
        self.pending = false;
        self.visible = true;
        true
    }

    pub(crate) fn visible(&self) -> bool {
        self.visible
    }
}

impl Default for LoadingState {
    fn default() -> Self {
        Self {
            started_at: None,
            delay: crate::ui::constants::LOADING_INDICATOR_DELAY,
            pending: false,
            visible: false,
            placeholder_pending: false,
        }
    }
}

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
pub(crate) enum DiffListMode {
    Files,
    Commits,
}
