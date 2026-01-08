use crate::state::{MergeStatus, PullRequestInfo};

#[derive(Clone, Copy)]
pub(crate) struct PrInfoRef<'a> {
    pr: &'a PullRequestInfo,
}

impl<'a> PrInfoRef<'a> {
    pub(crate) fn new(pr: &'a PullRequestInfo) -> Self {
        Self { pr }
    }

    pub(crate) fn number(self) -> i64 {
        self.pr.number
    }

    pub(crate) fn url(self) -> Option<&'a str> {
        let u = self.pr.url.as_str();
        (!u.trim().is_empty()).then_some(u)
    }

    pub(crate) fn status(self) -> MergeStatus {
        self.pr.status
    }

    pub(crate) fn badge(self) -> (i64, MergeStatus) {
        (self.number(), self.status())
    }
}
