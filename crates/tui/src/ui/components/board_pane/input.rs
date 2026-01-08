use crossterm::event::{KeyCode, MouseEventKind};

use super::{BoardPane, BoardPaneEvent, hit_test::board_hit_at};
use crate::{
    prefs::save_prefs,
    selection::find_task,
    state::{AppState, ConfirmAction, ConfirmAltAction, ConfirmState, DeleteTaskMode},
};

pub(super) fn handle_event(app: &mut AppState, event: BoardPaneEvent) -> bool {
    match event {
        BoardPaneEvent::Mouse { mouse, area } => {
            let col = mouse.column;
            let row = mouse.row;

            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    let Some(hit) = board_hit_at(app, area, col, row) else {
                        return false;
                    };
                    app.ui.focus_board();
                    crate::actions::selection::focus_board_section(app, hit.status);
                    crate::actions::selection::select_adjacent_task(app, -1);
                    true
                }
                MouseEventKind::ScrollDown => {
                    let Some(hit) = board_hit_at(app, area, col, row) else {
                        return false;
                    };
                    app.ui.focus_board();
                    crate::actions::selection::focus_board_section(app, hit.status);
                    crate::actions::selection::select_adjacent_task(app, 1);
                    true
                }
                MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                    app.ui.focus_board();
                    if let Some(hit) = board_hit_at(app, area, col, row) {
                        crate::actions::selection::apply_board_hit(app, hit);
                    }
                    true
                }
                _ => false,
            }
        }
        BoardPaneEvent::Key(key) => match key.code {
            KeyCode::Char('c') => {
                app.board.show_cancelled = !app.board.show_cancelled;
                crate::actions::selection::normalize_after_cancelled_toggle(app);
                app.prefs.show_cancelled = app.board.show_cancelled;
                save_prefs(&app.prefs);
                true
            }
            KeyCode::Char('n') => {
                crate::ui::open_create_task_modal(app, None);
                true
            }
            KeyCode::Char('N') => {
                crate::ui::open_create_task_modal(app, app.board.selected_task_id);
                true
            }
            KeyCode::Char('d') => {
                BoardPane::open_delete_task_confirm(app);
                true
            }
            KeyCode::Char('K') => {
                crate::actions::selection::move_active_status(app, -1);
                true
            }
            KeyCode::Char('J') => {
                crate::actions::selection::move_active_status(app, 1);
                true
            }
            KeyCode::Up | KeyCode::Char('k') => {
                crate::actions::selection::select_adjacent_task(app, -1);
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                crate::actions::selection::select_adjacent_task(app, 1);
                true
            }
            KeyCode::Left => {
                crate::actions::selection::request_move_selected_task(app, -1);
                true
            }
            KeyCode::Right => {
                crate::actions::selection::request_move_selected_task(app, 1);
                true
            }
            _ => false,
        },
    }
}

impl BoardPane {
    pub(super) fn open_delete_task_confirm(app: &mut AppState) {
        let Some(task_id) = app.board.selected_task_id else {
            app.ui.set_error("No task selected.");
            return;
        };
        let title = find_task(&app.board.tasks_store, task_id)
            .map(|t| t.title)
            .unwrap_or_else(|| task_id.to_string());

        let all_tasks = crate::selection::tasks_all(&app.board.tasks_store);
        let mut children_by_parent: std::collections::HashMap<uuid::Uuid, Vec<uuid::Uuid>> =
            std::collections::HashMap::new();
        for t in &all_tasks {
            if let Some(pid) = t.parent_task_id {
                children_by_parent.entry(pid).or_default().push(t.id);
            }
        }

        let mut descendants = 0usize;
        let mut stack = children_by_parent
            .get(&task_id)
            .cloned()
            .unwrap_or_default();
        while let Some(cur) = stack.pop() {
            descendants += 1;
            if let Some(ch) = children_by_parent.get(&cur) {
                stack.extend(ch.iter().copied());
            }
        }

        let body = if descendants > 0 {
            format!(
                "Delete task '{title}'?\n\nThis task has {descendants} subtask(s).\n\n- y/Enter: delete (promote subtasks)\n- D: delete subtree (destructive)"
            )
        } else {
            format!("Delete task '{title}'?")
        };

        app.ui.confirm = Some(ConfirmState {
            title: "Delete task?".to_string(),
            body,
            action: ConfirmAction::DeleteTask {
                task_id,
                delete_mode: DeleteTaskMode::Promote,
            },
            alt_action: (descendants > 0).then_some(ConfirmAltAction {
                key: 'D',
                label: "delete subtree".to_string(),
                action: ConfirmAction::DeleteTask {
                    task_id,
                    delete_mode: DeleteTaskMode::Subtree,
                },
            }),
        });
    }
}
