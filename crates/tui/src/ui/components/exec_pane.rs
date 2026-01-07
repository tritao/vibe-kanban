use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::{
    UiComponent,
    exec_input::{ExecInput, ExecInputEvent},
    exec_log::{ExecLog, ExecLogEvent},
};
use crate::{
    prefs::save_prefs,
    state::{AppState, FocusPane, LogRenderMode, LogViewMode},
};

pub(crate) enum ExecPaneEvent {
    Key(KeyEvent),
    Mouse { mouse: MouseEvent, area: Rect },
}

pub(crate) struct ExecPane;

impl UiComponent for ExecPane {
    type Event = ExecPaneEvent;

    fn render(f: &mut ratatui::Frame, app: &AppState, area: ratatui::layout::Rect) {
        crate::ui::render_execution_pane(f, app, area);
    }

    fn on_event(app: &mut AppState, event: Self::Event) -> bool {
        match event {
            ExecPaneEvent::Key(key) => handle_exec_key(app, key),
            ExecPaneEvent::Mouse { mouse, area } => handle_exec_mouse(app, mouse, area),
        }
    }
}

fn handle_exec_key(app: &mut AppState, key: KeyEvent) -> bool {
    if app.ui.focus != FocusPane::Execution {
        return false;
    }

    match key.code {
        KeyCode::Char('m') => {
            app.exec.log_render_mode = match app.exec.log_render_mode {
                LogRenderMode::Plain => LogRenderMode::Markdown,
                LogRenderMode::Markdown => LogRenderMode::Plain,
            };
            app.prefs.log_render_mode = app.exec.log_render_mode;
            save_prefs(&app.prefs);
            crate::logs::mark_all_log_buffers_dirty(app, 0);
            true
        }
        KeyCode::Char('v') => {
            app.exec.log_view_mode = match app.exec.log_view_mode {
                LogViewMode::Timeline => LogViewMode::Single,
                LogViewMode::Single => LogViewMode::Timeline,
            };
            app.prefs.log_view_mode = app.exec.log_view_mode;
            save_prefs(&app.prefs);
            app.exec.log_view_dirty = true;
            true
        }
        _ => <ExecLog as UiComponent>::on_event(app, ExecLogEvent::Key(key)),
    }
}

fn handle_exec_mouse(app: &mut AppState, mouse: MouseEvent, area: Rect) -> bool {
    let col = mouse.column;
    let row = mouse.row;
    let split = crate::layout::split_exec_pane(area);

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if crate::layout::rect_contains(split.logs, col, row) {
                app.ui.focus_execution();
                let _ = <ExecLog as UiComponent>::on_event(
                    app,
                    ExecLogEvent::WheelDelta(-crate::ui::constants::LOG_WHEEL_STEP),
                );
                return true;
            }
            false
        }
        MouseEventKind::ScrollDown => {
            if crate::layout::rect_contains(split.logs, col, row) {
                app.ui.focus_execution();
                let _ = <ExecLog as UiComponent>::on_event(
                    app,
                    ExecLogEvent::WheelDelta(crate::ui::constants::LOG_WHEEL_STEP),
                );
                return true;
            }
            false
        }
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            if crate::layout::rect_contains(split.input, col, row) {
                app.ui.focus_execution();
                if let Some(ExecInputEvent::ClickTo {
                    cursor,
                    content_w,
                    inner_h,
                }) = <ExecInput as UiComponent>::hit_test(app, split.input, col, row)
                {
                    return <ExecInput as UiComponent>::on_event(
                        app,
                        ExecInputEvent::ClickTo {
                            cursor,
                            content_w,
                            inner_h,
                        },
                    );
                }
                return true;
            }
            if crate::layout::rect_contains(split.logs, col, row) {
                app.ui.focus_execution();
                if let Some(evt) = <ExecLog as UiComponent>::hit_test(app, split.logs, col, row) {
                    return <ExecLog as UiComponent>::on_event(app, evt);
                }
                return true;
            }
            false
        }
        MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
            if crate::layout::rect_contains(split.logs, col, row) {
                app.ui.focus_execution();
                let Some(ExecLogEvent::Hit(_, hit)) =
                    <ExecLog as UiComponent>::hit_test(app, split.logs, col, row)
                else {
                    return false;
                };
                let Some(line_idx) = hit.line_idx else {
                    return false;
                };
                return <ExecLog as UiComponent>::on_event(app, ExecLogEvent::DragTo(line_idx));
            }
            false
        }
        MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
            if app.exec.log_mouse_selecting {
                return <ExecLog as UiComponent>::on_event(app, ExecLogEvent::DragEnd);
            }
            false
        }
        MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
            if crate::layout::rect_contains(split.logs, col, row) {
                app.ui.focus_execution();
                let Some(ExecLogEvent::Hit(_, hit)) =
                    <ExecLog as UiComponent>::hit_test(app, split.logs, col, row)
                else {
                    return false;
                };
                return <ExecLog as UiComponent>::on_event(
                    app,
                    ExecLogEvent::Hit(crate::ui::components::exec_log::ExecLogHitKind::Right, hit),
                );
            }
            false
        }
        _ => false,
    }
}
