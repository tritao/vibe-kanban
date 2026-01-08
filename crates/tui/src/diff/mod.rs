pub(crate) mod highlight;
pub(crate) mod preview;

pub(crate) use highlight::highlight_unified_diff;
pub(crate) use preview::{build_diff_preview_request, compute_diff_preview};

pub(crate) use crate::store::diff::DIFF_ALL_KEY;

pub(crate) fn diff_rows_with_all_filtered(
    store: &serde_json::Value,
    show_untracked: bool,
) -> Vec<crate::store::diff::DiffRow> {
    crate::store::diff::DiffStore::new(store).rows_with_all_filtered(show_untracked)
}
