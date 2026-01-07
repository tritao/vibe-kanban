#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Timestamp {
    pub(crate) dt: chrono::DateTime<chrono::Utc>,
    pub(crate) raw: String,
}

impl Timestamp {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        let raw = s.to_string();
        let dt = chrono::DateTime::parse_from_rfc3339(s)
            .ok()?
            .with_timezone(&chrono::Utc);
        Some(Self { dt, raw })
    }

    pub(crate) fn short_time(&self) -> String {
        self.dt.format("%H:%M:%S").to_string()
    }
}
