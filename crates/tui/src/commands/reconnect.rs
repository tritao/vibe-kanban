use crate::state::AppState;

pub(crate) fn request_reconnect_all(app: &mut AppState) {
    let next = *app.reconnect_tx.borrow() + 1;
    let _ = app.reconnect_tx.send(next);
}

pub(crate) fn request_diff_reconnect(app: &mut AppState) {
    let next = *app.diff_reconnect_tx.borrow() + 1;
    let _ = app.diff_reconnect_tx.send(next);
}
