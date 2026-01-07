use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecutorProfileSelection {
    pub(crate) executor: String,
    pub(crate) variant: Option<String>,
}
