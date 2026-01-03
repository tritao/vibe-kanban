pub(crate) mod highlight;
pub(crate) mod model;
pub(crate) mod preview;

pub(crate) use highlight::highlight_unified_diff;
pub(crate) use model::{diff_rows_with_all, DIFF_ALL_KEY};
pub(crate) use preview::{build_diff_preview_request, compute_diff_preview};
