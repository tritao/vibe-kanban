use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::state::{AppState, FocusPane};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ExecPaneRects {
    pub(crate) logs: Rect,
    pub(crate) input: Rect,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DiffPaneRects {
    pub(crate) repo_bar: Rect,
    pub(crate) files: Rect,
    pub(crate) preview: Rect,
}

pub(crate) fn split_exec_pane(area: Rect) -> ExecPaneRects {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(7)])
        .split(area);
    ExecPaneRects {
        logs: sections[0],
        input: sections[1],
    }
}

pub(crate) fn split_diff_pane(area: Rect) -> DiffPaneRects {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Min(3),
        ])
        .split(area);
    DiffPaneRects {
        repo_bar: sections[0],
        files: sections[1],
        preview: sections[2],
    }
}

pub(crate) fn current_terminal_rect() -> Rect {
    let (w, h) = crossterm::terminal::size().unwrap_or((80, 24));
    Rect {
        x: 0,
        y: 0,
        width: w,
        height: h,
    }
}

pub(crate) fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x
        && col < r.x.saturating_add(r.width)
        && row >= r.y
        && row < r.y.saturating_add(r.height)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MainLayoutRects {
    pub(crate) board: Rect,
    pub(crate) exec: Rect,
    pub(crate) diff: Rect,
    pub(crate) exec_logs: Rect,
    pub(crate) exec_input: Rect,
    pub(crate) diff_preview: Rect,
}

pub(crate) fn compute_main_layout(area: Rect, focus: FocusPane) -> MainLayoutRects {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    let (board_pct, exec_pct, diff_pct) = if focus == FocusPane::Diff {
        // Give the diff pane more room while it has focus for easier reviewing.
        (18, 46, 36)
    } else {
        (18, 54, 28)
    };
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(board_pct),
            Constraint::Percentage(exec_pct),
            Constraint::Percentage(diff_pct),
        ])
        .split(root[1]);

    let exec_split = split_exec_pane(main[1]);
    let diff_split = split_diff_pane(main[2]);

    MainLayoutRects {
        board: main[0],
        exec: main[1],
        diff: main[2],
        exec_logs: exec_split.logs,
        exec_input: exec_split.input,
        diff_preview: diff_split.preview,
    }
}

pub(crate) fn clamp_scroll_offsets(app: &mut AppState, layout: MainLayoutRects) -> bool {
    // Prevent internal offsets from growing beyond the maximum (overscroll), which would require
    // scrolling back down the same amount before the viewport starts moving again.
    let mut changed = false;

    // Execution log pane (offset counts "lines above the viewport", i.e. distance from bottom).
    {
        let len = app.exec.log_lines.len();
        let height = layout.exec_logs.height.saturating_sub(2) as usize;
        let visible = height.min(len);
        let before = app.exec.log_scroll;
        let mut next = before;
        next.normalize(len, visible);
        if next.offset_from_end != before.offset_from_end || next.autoscroll != before.autoscroll {
            app.exec.log_scroll = next;
            changed = true;
        }
    }

    // Diff preview pane (offset counts "first visible line").
    {
        let len = app.diff.diff_preview_lines.len();
        let height = layout.diff_preview.height.saturating_sub(2) as usize;
        let visible = height.min(len);
        let before = app.diff.diff_scroll;
        let mut next = before;
        next.normalize(len, visible);
        if next.offset != before.offset {
            app.diff.diff_scroll = next;
            changed = true;
        }
    }

    changed
}
