use std::time::{Duration, Instant};

use ratatui::style::Color;

use super::app_state::UiState;
use crate::state::{DiffFocus, FocusPane, ToastState, UiMessage, UiMessageKey, UiMessageKind};

impl UiState {
    pub(crate) fn focus_board(&mut self) {
        self.focus = FocusPane::Board;
    }

    pub(crate) fn focus_execution(&mut self) {
        self.focus = FocusPane::Execution;
    }

    pub(crate) fn focus_diff(&mut self) {
        self.focus = FocusPane::Diff;
    }

    pub(crate) fn focus_diff_files(&mut self) {
        self.focus = FocusPane::Diff;
        self.diff_focus = DiffFocus::Files;
    }

    pub(crate) fn focus_diff_preview(&mut self) {
        self.focus = FocusPane::Diff;
        self.diff_focus = DiffFocus::Preview;
    }

    pub(crate) fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            FocusPane::Board => FocusPane::Execution,
            FocusPane::Execution => FocusPane::Diff,
            FocusPane::Diff => FocusPane::Board,
        };
    }

    pub(crate) fn set_notice(&mut self, msg: impl Into<String>) {
        self.last_notice = Some(UiMessage {
            kind: UiMessageKind::Notice,
            scope: None,
            text: msg.into(),
            created_at: Instant::now(),
        });
    }

    pub(crate) fn set_error(&mut self, msg: impl Into<String>) {
        self.last_error = Some(UiMessage {
            kind: UiMessageKind::Error,
            scope: None,
            text: msg.into(),
            created_at: Instant::now(),
        });
        if let Some(state) = self.project_setup.as_mut() {
            state.busy = false;
        }
    }

    pub(crate) fn set_error_key(&mut self, key: UiMessageKey, msg: impl Into<String>) {
        self.last_error = Some(UiMessage {
            kind: UiMessageKind::Error,
            scope: Some(key),
            text: msg.into(),
            created_at: Instant::now(),
        });
        if let Some(state) = self.project_setup.as_mut() {
            state.busy = false;
        }
    }

    pub(crate) fn clear_messages(&mut self) {
        self.last_error = None;
        self.last_notice = None;
    }

    pub(crate) fn clear_error_scope(&mut self, scope: UiMessageKey) -> bool {
        if let Some(err) = self.last_error.as_ref()
            && err.scope == Some(scope)
        {
            self.last_error = None;
            return true;
        }
        false
    }

    pub(crate) fn set_toast(
        &mut self,
        message: impl Into<String>,
        color: Color,
        expires_in: Option<Duration>,
    ) {
        self.toast = Some(ToastState {
            message: message.into(),
            color,
            expires_at: expires_in.map(|d| Instant::now() + d),
        });
    }
}
