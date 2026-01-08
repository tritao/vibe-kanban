use std::collections::HashMap;

use uuid::Uuid;

use super::LoadingState;

#[derive(Debug, Clone)]
pub(crate) struct RepoListState<T> {
    pub(crate) items: Vec<T>,
    pub(crate) loading: LoadingState,
    pub(crate) has_more: bool,
}

impl<T> Default for RepoListState<T> {
    fn default() -> Self {
        Self {
            items: vec![],
            loading: LoadingState::default(),
            has_more: true,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RepoLatestState<T> {
    pub(crate) values: HashMap<Uuid, T>,
    pub(crate) generations: crate::jobs::latest::LatestByKey<Uuid>,
}

impl<T> Default for RepoLatestState<T> {
    fn default() -> Self {
        Self {
            values: HashMap::new(),
            generations: crate::jobs::latest::LatestByKey::default(),
        }
    }
}
