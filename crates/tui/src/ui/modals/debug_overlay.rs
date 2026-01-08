use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::component::ModalComponent;
use crate::state::{AppState, JobKey};

pub(super) static OVERLAY: DebugOverlay = DebugOverlay;

pub(super) struct DebugOverlay;

fn job_label(k: &JobKey) -> &'static str {
    match k {
        JobKey::DiffPreview => "DiffPreview",
        JobKey::LogPrewarm => "LogPrewarm",
        JobKey::BranchStatus => "BranchStatus",
        JobKey::BranchStatusAuto => "BranchStatusAuto",
        JobKey::CommitList => "CommitList",
        JobKey::CommitPreview => "CommitPreview",
        JobKey::PullRequestCreate => "PullRequestCreate",
        JobKey::PullRequestAttach => "PullRequestAttach",
        JobKey::PullRequestComments => "PullRequestComments",
        JobKey::OpenEditor => "OpenEditor",
        JobKey::ExecutorProfile => "ExecutorProfile",
        JobKey::ModelSettings => "ModelSettings",
        JobKey::StackStatus => "StackStatus",
        JobKey::TaskDelete => "TaskDelete",
    }
}

impl ModalComponent for DebugOverlay {
    fn is_open(&self, app: &AppState) -> bool {
        app.ui.debug_overlay
    }

    fn blocks_mouse(&self, _app: &AppState) -> bool {
        false
    }

    fn render(&self, f: &mut Frame, app: &AppState) {
        let running: Vec<&JobKey> = app
            .jobs
            .iter()
            .filter(|(_, h)| !h.is_finished())
            .map(|(k, _)| k)
            .collect();

        let mut lines: Vec<Line<'static>> = vec![];
        lines.push(Line::from(vec![
            Span::styled("Focus: ", Style::default().add_modifier(Modifier::DIM)),
            Span::raw(format!("{:?}", app.ui.focus)),
            Span::raw("  "),
            Span::styled("DiffFocus: ", Style::default().add_modifier(Modifier::DIM)),
            Span::raw(format!("{:?}", app.ui.diff_focus)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Jobs: ", Style::default().add_modifier(Modifier::DIM)),
            Span::raw(format!("{} running", running.len())),
        ]));
        if !running.is_empty() {
            let mut job_names: Vec<&'static str> = running.iter().map(|k| job_label(k)).collect();
            job_names.sort();
            job_names.dedup();
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default().add_modifier(Modifier::DIM)),
                Span::raw(job_names.join(", ")),
            ]));
        }
        lines.push(Line::from(vec![
            Span::styled("Diff: ", Style::default().add_modifier(Modifier::DIM)),
            Span::raw(format!(
                "pending={}, gen={}, cache_key={}",
                app.diff.diff_preview_pending,
                app.diff.diff_preview_gen.current(),
                app.diff
                    .diff_preview_cache_key
                    .as_deref()
                    .unwrap_or("<none>")
            )),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Exec: ", Style::default().add_modifier(Modifier::DIM)),
            Span::raw(format!(
                "view_dirty={}, width={}->{}, mode={:?}/{:?}",
                app.exec.log_view_dirty,
                app.exec.log_render_width,
                app.exec.log_target_render_width,
                app.exec.log_mode,
                app.exec.log_render_mode
            )),
        ]));

        let block = Block::default()
            .title(Span::styled(
                "Debug",
                Style::default().add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL);

        // Place it at the top-right; keep it compact.
        let root = f.area();
        let width = (root.width / 2).max(30).min(80);
        let height = (lines.len() as u16 + 2)
            .min(root.height.saturating_sub(1))
            .max(3);

        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(width)])
            .split(root);
        let area = *cols.last().unwrap_or(&root);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(height), Constraint::Min(0)])
            .split(area);
        let area = rows[0];

        let p = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
        f.render_widget(p, area);
    }

    fn on_key(&self, _app: &mut AppState, _key: KeyEvent) -> bool {
        false
    }

    fn on_mouse(&self, _app: &mut AppState, _mouse: MouseEvent) -> bool {
        false
    }
}
