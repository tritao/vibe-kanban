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
