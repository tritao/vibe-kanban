use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    state::{AppState, ConfirmState},
    ui::layout::centered_rect,
};

pub(crate) fn render_confirm_modal(f: &mut Frame, confirm: &ConfirmState) {
    let area = centered_rect(70, 35, f.area());
    f.render_widget(Clear, area);

    let mut lines = vec![
        Line::from(vec![Span::styled(
            confirm.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(confirm.body.clone()),
        Line::from(""),
    ];
    if let Some(alt) = confirm.alt_action.as_ref() {
        lines.push(Line::from(format!(
            "y/Enter = confirm, {} = {}, n/Esc = cancel",
            alt.key, alt.label
        )));
    } else {
        lines.push(Line::from("y/Enter = confirm, n/Esc = cancel"));
    }

    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Confirm"))
        .wrap(Wrap { trim: true });
    f.render_widget(p, area);
}

pub(crate) fn handle_confirm_key(app: &mut AppState, key: KeyEvent) -> bool {
    let Some(confirm) = app.ui.confirm.as_ref() else {
        return false;
    };

    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => {
            let action = confirm.action;
            app.ui.confirm = None;
            crate::handle_confirm_action(app, action);
            true
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            app.ui.confirm = None;
            true
        }
        KeyCode::Char(c) => {
            if let Some(alt) = confirm.alt_action.as_ref().filter(|alt| alt.key == c) {
                let action = alt.action;
                app.ui.confirm = None;
                crate::handle_confirm_action(app, action);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}
