use super::modals;
use crate::{
    selection::find_task,
    state::{AppState, ConfirmAction, ConfirmAltAction, ConfirmState, DeleteTaskMode},
};

pub(super) fn handle_confirm_key(app: &mut AppState, key: crossterm::event::KeyEvent) -> bool {
    let Some(confirm) = app.ui.confirm.as_ref() else {
        return false;
    };

    match key.code {
        crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Enter => {
            let action = confirm.action;
            modals::close_confirm(app);
            crate::handle_confirm_action(app, action);
            true
        }
        crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Esc => {
            modals::close_confirm(app);
            true
        }
        crossterm::event::KeyCode::Char(c) => {
            if let Some(alt) = confirm.alt_action.as_ref().filter(|alt| alt.key == c) {
                let action = alt.action;
                modals::close_confirm(app);
                crate::handle_confirm_action(app, action);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

pub(super) fn open_stop_exec_confirm(app: &mut AppState) -> bool {
    let Some(exec_id) = app.exec.selected_exec_id else {
        return false;
    };

    modals::open_confirm(
        app,
        ConfirmState {
            title: "Stop execution?".to_string(),
            body: format!("Stop execution process {exec_id}? (y/n)"),
            action: ConfirmAction::StopExec { exec_id },
            alt_action: None,
        },
    );
    true
}

pub(super) fn open_delete_task_confirm(app: &mut AppState) -> bool {
    let Some(task_id) = app.board.selected_task_id else {
        app.ui.last_error = Some("No task selected.".to_string());
        return true;
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

    modals::open_confirm(
        app,
        ConfirmState {
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
        },
    );
    true
}
