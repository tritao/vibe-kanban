use uuid::Uuid;

use crate::state::TaskRow;

pub(crate) fn task_index_in(list: &[TaskRow], selected_id: Option<Uuid>) -> Option<usize> {
    let selected_id = selected_id?;
    list.iter().position(|t| t.id == selected_id)
}

pub(crate) fn clamp_index(cur: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta < 0 {
        cur.saturating_sub(delta.unsigned_abs() as usize)
    } else {
        (cur + delta as usize).min(len - 1)
    }
}
