use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::Frame;

use crate::state::AppState;

pub(crate) trait ModalComponent: Sync {
    fn is_open(&self, app: &AppState) -> bool;
    fn blocks_mouse(&self, app: &AppState) -> bool {
        self.is_open(app)
    }
    fn render(&self, f: &mut Frame, app: &AppState);
    fn on_key(&self, app: &mut AppState, key: KeyEvent) -> bool;
    fn on_mouse(&self, _app: &mut AppState, _mouse: MouseEvent) -> bool {
        false
    }
}
