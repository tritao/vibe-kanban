use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use super::component::ModalComponent;
use crate::{
    layout::current_terminal_rect,
    state::{AppState, InputMode, InputState, TextFieldState},
    text::{display_width, slice_by_display_cols},
    ui::layout::centered_rect,
};

pub(crate) struct InputModal;
pub(crate) static MODAL: InputModal = InputModal;

impl ModalComponent for InputModal {
    fn is_open(&self, app: &AppState) -> bool {
        app.ui.input.is_some()
    }

    fn render(&self, f: &mut Frame, app: &AppState) {
        if let Some(input) = app.ui.input.as_ref() {
            render_input_modal(f, input);
        }
    }

    fn on_key(&self, app: &mut AppState, key: KeyEvent) -> bool {
        handle_search_key(app, key)
    }

    fn on_mouse(&self, app: &mut AppState, mouse: MouseEvent) -> bool {
        handle_search_caret_click(app, mouse)
    }
}

pub(crate) fn open_search(app: &mut AppState) {
    let mut field = TextFieldState::default();
    field.buffer = app.board.task_filter.clone();
    field.set_end();
    let term = current_terminal_rect();
    let area = centered_rect(80, 25, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1);
    field.ensure_cursor_visible(content_w, 1);
    app.ui.input = Some(InputState {
        mode: InputMode::SearchTasks,
        field,
        original: app.board.task_filter.clone(),
    });
}

fn close_search(app: &mut AppState, restore_original: bool) {
    if let Some(input) = app.ui.input.take() {
        if restore_original {
            app.board.task_filter = input.original;
        }
    }
    crate::actions::selection::ensure_selection_visible(app);
}

fn close_search_keep(app: &mut AppState) {
    app.ui.input = None;
    crate::actions::selection::ensure_selection_visible(app);
}

pub(crate) fn handle_search_key(app: &mut AppState, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc => {
            close_search(app, true);
            return true;
        }
        KeyCode::Enter => {
            close_search_keep(app);
            return true;
        }
        _ => {}
    }

    let Some(mut input) = app.ui.input.take() else {
        return false;
    };

    if !crate::text::field_edit::apply_text_field_key(&mut input.field, key, false) {
        app.ui.input = Some(input);
        return false;
    }

    app.board.task_filter = input.field.buffer.clone();
    let term = current_terminal_rect();
    let area = centered_rect(80, 25, term);
    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1);
    input.field.ensure_cursor_visible(content_w, 1);
    crate::actions::selection::ensure_selection_visible(app);

    app.ui.input = Some(input);
    true
}

pub(crate) fn handle_search_caret_click(app: &mut AppState, mouse: MouseEvent) -> bool {
    let col = mouse.column;
    let row = mouse.row;
    let Some(input) = app.ui.input.as_mut() else {
        return false;
    };
    if app.ui.confirm.is_some() || app.ui.show_help || app.ui.create_task.is_some() {
        return false;
    }
    if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
        return false;
    }

    let area = centered_rect(80, 25, current_terminal_rect());
    let input_y = area.y.saturating_add(1).saturating_add(2);
    let input_x0 = area.x.saturating_add(1).saturating_add(1);
    let input_x1 = area.x.saturating_add(area.width).saturating_sub(2);

    if row != input_y || col < input_x0 || col > input_x1 {
        return false;
    }

    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1);

    let start_col = input.field.scroll_x as usize;
    let left = start_col > 0;
    let click_x = col.saturating_sub(input_x0) as usize;
    let mut target_col = if left {
        if click_x == 0 {
            start_col
        } else {
            start_col.saturating_add(click_x.saturating_sub(1))
        }
    } else {
        start_col.saturating_add(click_x)
    };

    let line_w = display_width(&input.field.buffer);
    target_col = target_col.min(line_w);
    input.field.cursor =
        crate::text::edit::byte_index_at_display_col(&input.field.buffer, target_col);
    input.field.goal_col = None;
    input.field.ensure_cursor_visible(content_w, 1);
    true
}

pub(crate) fn render_input_modal(f: &mut Frame, input: &InputState) {
    let area = centered_rect(80, 25, f.area());
    f.render_widget(Clear, area);

    let (title, hint) = match input.mode {
        InputMode::SearchTasks => (
            "Search tasks",
            "type to filter, Enter to apply, Esc to cancel",
        ),
    };

    // Single-line editor with horizontal scroll.
    let inner_w = area.width.saturating_sub(2) as usize;
    let content_w = inner_w.saturating_sub(1).saturating_sub(1).max(1); // "/" + free cell
    let start_col = input.field.scroll_x as usize;
    let left = start_col > 0;

    let buf = input.field.buffer.as_str();
    let buf_w = display_width(buf);
    let mut right = false;
    let mut take = content_w.saturating_sub(left as usize);
    if start_col.saturating_add(take) < buf_w {
        right = true;
        take = content_w.saturating_sub(left as usize).saturating_sub(1);
    }

    let mut visible = String::new();
    if left {
        visible.push('…');
    }
    visible.push_str(&slice_by_display_cols(buf, start_col, take));
    if right {
        visible.push('…');
    }

    let lines = vec![
        Line::from(vec![Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(format!("/{visible}")),
        Line::from(""),
        Line::from(hint),
    ];

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);

    // Cursor position: title (0), blank (1), input (2).
    let (_, col) = input.field.cursor_line_col();
    let cursor_in_chunk = col.saturating_sub(start_col).min(take);
    let cursor_x_in_visible = (left as usize).saturating_add(cursor_in_chunk);

    let x = area
        .x
        .saturating_add(1)
        .saturating_add(1) // leading "/"
        .saturating_add(cursor_x_in_visible as u16)
        .min(area.x.saturating_add(area.width).saturating_sub(2));
    let y = area.y.saturating_add(1).saturating_add(2);
    f.set_cursor_position((x, y));
}
