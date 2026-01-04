use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::state::{AppState, FocusPane};

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
    #[allow(dead_code)]
    pub(crate) diff_repo_bar: Rect,
    pub(crate) diff_files: Rect,
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

    let exec_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(7)])
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

pub(crate) fn clamp_scroll_offsets(app: &mut AppState, layout: MainLayoutRects) -> bool {
    // Prevent internal offsets from growing beyond the maximum (overscroll), which would require
    // scrolling back down the same amount before the viewport starts moving again.
    let mut changed = false;

    // Execution log pane (offset counts "lines above the viewport", i.e. distance from bottom).
    {
        let len = app.exec.log_lines.len();
        let height = layout.exec_logs.height.saturating_sub(2) as usize;
        let visible = height.min(len);
        let max_offset = len.saturating_sub(visible);

        if app.exec.log_autoscroll {
            if app.exec.log_scroll_offset != 0 {
                app.exec.log_scroll_offset = 0;
                changed = true;
            }
        } else {
            let next = app.exec.log_scroll_offset.min(max_offset);
            if next != app.exec.log_scroll_offset {
                app.exec.log_scroll_offset = next;
                changed = true;
            }
            if app.exec.log_scroll_offset == 0 && !app.exec.log_autoscroll {
                app.exec.log_autoscroll = true;
                changed = true;
            }
        }
    }

    // Diff preview pane (offset counts "first visible line").
    {
        let len = app.diff.diff_preview_lines.len();
        let height = layout.diff_preview.height.saturating_sub(2) as usize;
        let visible = height.min(len);
        let max_start = len.saturating_sub(visible);

        let next = app.diff.diff_scroll_offset.min(max_start);
        if next != app.diff.diff_scroll_offset {
            app.diff.diff_scroll_offset = next;
            changed = true;
        }
    }

    changed
}
