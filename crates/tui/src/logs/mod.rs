pub(crate) mod markdown;
pub(crate) mod assemble;
pub(crate) mod buffer;

pub(crate) use buffer::{
    enqueue_log_patch, flush_log_buffers, mark_all_log_buffers_dirty, reset_logs,
    toggle_selected_log_entry, ExecLogBuffer, LogSelection,
};
