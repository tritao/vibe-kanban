pub(crate) mod markdown;
pub(crate) mod assemble;
pub(crate) mod buffer;

pub(crate) use buffer::{
    append_local_user_message, enqueue_log_patch, flush_log_buffers, mark_all_log_buffers_dirty,
    maybe_attach_pending_user_log, prewarm_log_cache_for_exec, reset_logs, set_pending_user_log,
    toggle_selected_log_entry, ExecLogBuffer, LogSelection,
};
