#[derive(Debug, Clone)]
pub(crate) struct StackPatchEntry {
    pub(crate) name: String,
    #[allow(dead_code)]
    pub(crate) description: Option<String>,
    pub(crate) state: String,
    pub(crate) is_current: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct StackStatusResponse {
    pub(crate) available: bool,
    pub(crate) enabled: bool,
    pub(crate) patches: Vec<StackPatchEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct CommitEntry {
    pub(crate) oid: String,
    pub(crate) short_oid: String,
    #[allow(dead_code)]
    pub(crate) unix_ts: i64,
    pub(crate) subject: String,
}
