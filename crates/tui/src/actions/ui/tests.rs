use crate::{
    events::UiEvent,
    state::{DiffFocus, FocusPane, InputMode, InputState, TextFieldState},
};

#[test]
fn copy_diff_files_emits_copy_effect() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = crate::test_support::mk_app();
    app.ui.focus = FocusPane::Diff;
    app.ui.diff_focus = DiffFocus::Files;
    app.diff.diff_store = serde_json::json!({
        "entries": {
            "a.txt": { "type": "DIFF", "content": { "change": "modified", "additions": 1, "deletions": 0 } }
        }
    });
    app.diff.selected_diff_index = 1; // 0 is ALL
    crate::selection::change::on_diff_file_selected(&mut app);

    let res = super::reduce_ui(
        &mut app,
        UiEvent::Crossterm(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('y'),
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))),
    )
    .unwrap();
    assert!(!res.should_quit);
    assert!(
        res.effects
            .iter()
            .any(|e| matches!(e, super::Effect::CopyOsc52(s) if s == "a.txt"))
    );
}

#[test]
fn composer_left_moves_cursor() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = crate::test_support::mk_app();
    app.ui.focus = FocusPane::Execution;
    app.ui.composer_active = true;
    app.ui.composer.buffer = "hey".to_string();
    app.ui.composer.set_end();

    let res = super::reduce_ui(
        &mut app,
        UiEvent::Crossterm(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Left,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))),
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(app.ui.composer.cursor, 2);
}

#[test]
fn search_char_updates_filter() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = crate::test_support::mk_app();
    app.board.task_filter = "ab".to_string();
    let mut field = TextFieldState::default();
    field.buffer = app.board.task_filter.clone();
    field.set_end();
    app.ui.input = Some(InputState {
        mode: InputMode::SearchTasks,
        field,
        original: app.board.task_filter.clone(),
    });

    let res = super::reduce_ui(
        &mut app,
        UiEvent::Crossterm(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('c'),
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))),
    )
    .unwrap();
    assert!(res.changed);
    assert_eq!(app.board.task_filter, "abc");
}

#[test]
fn composer_enter_applies_slash_autocomplete_instead_of_submit() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = crate::test_support::mk_app();
    app.ui.focus = FocusPane::Execution;
    app.ui.composer_active = true;
    app.ui.composer.buffer = "/he".to_string();
    app.ui.composer.set_end();

    let res = super::reduce_ui(
        &mut app,
        UiEvent::Crossterm(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))),
    )
    .unwrap();

    assert!(!res.should_quit);
    assert!(res.changed);
    assert_eq!(app.ui.composer.buffer, "/help ");
    assert!(app.ui.composer_active);
}

#[test]
fn composer_enter_with_no_attempt_keeps_composer_open() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = crate::test_support::mk_app();
    app.ui.focus = FocusPane::Execution;
    app.ui.composer_active = true;
    app.ui.composer.buffer = "hello".to_string();
    app.ui.composer.set_end();
    app.board.selected_attempt_id = None;
    app.exec.exec_store = crate::store::exec::empty_exec_store();

    let res = super::reduce_ui(
        &mut app,
        UiEvent::Crossterm(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))),
    )
    .unwrap();

    assert!(!res.should_quit);
    assert!(res.changed);
    assert!(app.ui.composer_active);
    assert_eq!(app.ui.composer.buffer, "hello");
    assert!(
        app.ui
            .last_error
            .as_ref()
            .map(|m| m.text.as_str())
            .unwrap_or("")
            .contains("No task/attempt selected")
    );
}

#[test]
fn text_field_allows_shift_char_insertion() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = crate::test_support::mk_app();
    app.ui.create_task = Some(crate::state::CreateTaskState {
        title: Default::default(),
        description: Default::default(),
        status: crate::state::TaskStatus::Todo,
        parent_task_id: None,
        focus: crate::state::CreateTaskFocus::Description,
        selected_button: 0,
        error: None,
    });
    let state = app.ui.create_task.as_mut().unwrap();
    state.focus = crate::state::CreateTaskFocus::Description;

    let res = super::reduce_ui(
        &mut app,
        UiEvent::Crossterm(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('N'),
            KeyModifiers::SHIFT,
            KeyEventKind::Press,
        ))),
    )
    .unwrap();

    assert!(res.changed);
    let state = app.ui.create_task.as_ref().unwrap();
    assert_eq!(state.description.buffer, "N");
}
