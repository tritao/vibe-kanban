use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::{
    layout::compute_main_layout,
    state::AppState,
    ui::{
        components::{BoardPane, DiffPane, ExecPane, UiComponent},
        render_bottom_bar, render_composer_autocomplete, render_top_bar,
    },
};

pub(crate) fn render(f: &mut Frame, app: &AppState) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(f.area());
    let layout = compute_main_layout(f.area(), app.ui.focus);

    let top = render_top_bar(app);
    f.render_widget(top, root[0]);

    <BoardPane as UiComponent>::render(f, app, layout.board);
    <ExecPane as UiComponent>::render(f, app, layout.exec);
    <DiffPane as UiComponent>::render(f, app, layout.diff);

    let bottom = render_bottom_bar(app);
    f.render_widget(bottom, root[2]);

    render_composer_autocomplete(f, app, layout.exec_input);

    crate::ui::modals::render_overlays(f, app);
}
