use std::sync::OnceLock;

use anyhow::Context;
use serde::de::DeserializeOwned;
use utils::response::ApiResponse;

static HTTP_CLIENT: OnceLock<anyhow::Result<reqwest::Client>> = OnceLock::new();

pub(crate) fn http_client() -> anyhow::Result<&'static reqwest::Client> {
    match HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .build()
            .context("build reqwest client")
    }) {
        Ok(client) => Ok(client),
        Err(e) => Err(anyhow::anyhow!(e.to_string())),
    }
}

pub(crate) fn url(base_url: &str, path: &str) -> String {
    format!("{}{}", base_url.trim_end_matches('/'), path)
}

pub(crate) async fn decode_json_response<T: DeserializeOwned>(
    resp: reqwest::Response,
) -> anyhow::Result<T> {
    let status = resp.status();
    let body = resp.text().await.context("read response body")?;
    serde_json::from_str::<T>(&body).with_context(|| {
        let mut snippet = body.trim().to_string();
        const MAX: usize = 800;
        if snippet.len() > MAX {
            snippet.truncate(MAX);
            snippet.push_str("…");
        }
        if snippet.is_empty() {
            snippet = "<empty body>".to_string();
        }
        format!("decode response body as JSON (status {status}): {snippet}")
    })
}

pub(crate) async fn decode_api_response<T: DeserializeOwned>(
    resp: reqwest::Response,
) -> anyhow::Result<ApiResponse<T>> {
    decode_json_response(resp).await
}
