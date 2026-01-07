#[derive(Debug, serde::Deserialize)]
pub(crate) struct ApiResponseWire<T, E = serde_json::Value> {
    pub(crate) success: bool,
    pub(crate) data: Option<T>,
    pub(crate) error_data: Option<E>,
    pub(crate) message: Option<String>,
}
