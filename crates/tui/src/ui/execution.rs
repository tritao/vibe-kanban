use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
};

use super::components::{UiComponent, exec_input::ExecInput, exec_log::ExecLog};
use crate::state::AppState;

pub(crate) fn render_execution_pane(f: &mut Frame, app: &AppState, area: Rect) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(7)])
        .split(area);

    <ExecLog as UiComponent>::render(f, app, sections[0]);
    <ExecInput as UiComponent>::render(f, app, sections[1]);
}

pub(crate) fn render_composer_autocomplete(f: &mut Frame, app: &AppState, input_area: Rect) {
    if !app.ui.composer_active || !crate::slash::composer_is_slash_mode(&app.ui.composer.buffer) {
        return;
    }

    let items = crate::slash::composer_completion_items(app);
    if items.is_empty() {
        return;
    }

    let max_items = 6usize;
    let visible = items.len().min(max_items);
    let height = (visible + 2).min(input_area.y as usize);
    if height < 3 {
        return;
    }
    let height_u16 = height as u16;
    let y = input_area.y.saturating_sub(height_u16);
    let area = Rect {
        x: input_area.x,
        y,
        width: input_area.width,
        height: height_u16,
    };

    f.render_widget(Clear, area);

    let start = app
        .ui
        .composer_suggest_index
        .saturating_sub(visible.saturating_sub(1));
    let end = (start + visible).min(items.len());
    let window = &items[start..end];

    let list_items: Vec<ListItem> = window
        .iter()
        .cloned()
        .map(|it| {
            let mut spans: Vec<Span<'static>> = vec![Span::styled(
                it.insert.trim().to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            )];
            if !it.desc.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    it.desc,
                    Style::default().add_modifier(Modifier::DIM),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut state = ratatui::widgets::ListState::default();
    let selected_in_window = app.ui.composer_suggest_index.saturating_sub(start);
    state.select(Some(
        selected_in_window.min(list_items.len().saturating_sub(1)),
    ));

    let w = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Commands")
                .border_style(crate::ui::palette::border_active()),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");

    f.render_stateful_widget(w, area, &mut state);
}
