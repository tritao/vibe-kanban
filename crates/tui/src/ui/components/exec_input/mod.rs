use ratatui::{Frame, layout::Rect};

use super::UiComponent;
use crate::{
    layout::rect_contains,
    state::{AppState, FocusPane},
};

mod hit_test;
mod render;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExecInputEvent {
    ClickTo {
        cursor: usize,
        content_w: usize,
        inner_h: usize,
    },
}

pub(crate) struct ExecInput;

impl UiComponent for ExecInput {
    type Event = ExecInputEvent;

    fn render(f: &mut Frame, app: &AppState, area: Rect) {
        render::render_exec_input(f, app, area);
    }

    fn hit_test(app: &AppState, area: Rect, col: u16, row: u16) -> Option<Self::Event> {
        if !rect_contains(area, col, row) {
            return None;
        }
        Some(hit_test::compute_click_cursor(app, area, col, row))
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        if app.ui.focus != FocusPane::Execution {
            return false;
        }

        match event {
            ExecInputEvent::ClickTo {
                cursor,
                content_w,
                inner_h,
            } => {
                app.ui.composer_active = true;
                app.ui.composer_suggest_index = 0;
                app.ui.composer.cursor = cursor.min(app.ui.composer.buffer.len());
                app.ui.composer.goal_col = None;
                app.ui.composer.ensure_cursor_visible(content_w, inner_h);
                true
            }
        }
    }
}
