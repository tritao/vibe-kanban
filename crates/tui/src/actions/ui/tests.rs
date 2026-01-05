use tokio::sync::{mpsc, watch};

use crate::{
    events::{NetEvent, UiEvent},
    state::{AppState, DiffFocus, FocusPane, InputMode, InputState, LogMode, TextFieldState},
};

fn mk_app() -> AppState {
    let (net_tx, _net_rx) = mpsc::channel::<NetEvent>(8);
    let (project_sel_tx, _project_sel_rx) = watch::channel(None);
    let (attempt_sel_tx, _attempt_sel_rx) = watch::channel(None);
    let (exec_sel_tx, _exec_sel_rx) = watch::channel(None);
    let (log_mode_tx, _log_mode_rx) = watch::channel(LogMode::Normalized);
    let (diff_stats_tx, _diff_stats_rx) = watch::channel(false);
    let (diff_reconnect_tx, _diff_reconnect_rx) = watch::channel(0u64);
    let (reconnect_tx, _reconnect_rx) = watch::channel(0u64);

    AppState::new(
        "http://127.0.0.1:1234".to_string(),
        net_tx,
        project_sel_tx,
        attempt_sel_tx,
        exec_sel_tx,
        log_mode_tx,
        diff_stats_tx,
        diff_reconnect_tx,
        reconnect_tx,
        crate::state::TuiPrefs::default(),
    )
}

#[test]
fn copy_diff_files_emits_copy_effect() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = mk_app();
    app.ui.focus = FocusPane::Diff;
    app.ui.diff_focus = DiffFocus::Files;
    app.diff.diff_store = serde_json::json!({
        "entries": {
            "a.txt": { "type": "DIFF", "content": { "change": "modified", "additions": 1, "deletions": 0 } }
        }
    });
    super::sel::select_diff_file(&mut app, 1); // 0 is ALL

    let (quit, _dirty, effects) = super::reduce_ui(
        &mut app,
        UiEvent::Crossterm(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('y'),
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))),
    )
    .unwrap();
    assert!(!quit);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, super::Effect::CopyOsc52(s) if s == "a.txt"))
    );
}

#[test]
fn composer_left_moves_cursor() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = mk_app();
    app.ui.focus = FocusPane::Execution;
    app.ui.composer_active = true;
    app.ui.composer.buffer = "hey".to_string();
    app.ui.composer.set_end();

    let (_quit, dirty, _effects) = super::reduce_ui(
        &mut app,
        UiEvent::Crossterm(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Left,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))),
    )
    .unwrap();
    assert!(dirty);
    assert_eq!(app.ui.composer.cursor, 2);
}

#[test]
fn search_char_updates_filter() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = mk_app();
    app.board.task_filter = "ab".to_string();
    let mut field = TextFieldState::default();
    field.buffer = app.board.task_filter.clone();
    field.set_end();
    app.ui.input = Some(InputState {
        mode: InputMode::SearchTasks,
        field,
        original: app.board.task_filter.clone(),
    });

    let (_quit, dirty, _effects) = super::reduce_ui(
        &mut app,
        UiEvent::Crossterm(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('c'),
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))),
    )
    .unwrap();
    assert!(dirty);
    assert_eq!(app.board.task_filter, "abc");
}

#[test]
fn composer_enter_applies_slash_autocomplete_instead_of_submit() {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    let mut app = mk_app();
    app.ui.focus = FocusPane::Execution;
    app.ui.composer_active = true;
    app.ui.composer.buffer = "/he".to_string();
    app.ui.composer.set_end();

    let (quit, dirty, _effects) = super::reduce_ui(
        &mut app,
        UiEvent::Crossterm(Event::Key(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        ))),
    )
    .unwrap();

    assert!(!quit);
    assert!(dirty);
    assert_eq!(app.ui.composer.buffer, "/help ");
    assert!(app.ui.composer_active);
}
