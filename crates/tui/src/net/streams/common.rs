#[derive(Debug)]
pub(crate) enum WsConnectError {
    HttpStatus(u16),
    Other(anyhow::Error),
}

impl std::fmt::Display for WsConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HttpStatus(code) => write!(f, "HTTP {code}"),
            Self::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for WsConnectError {}

pub(crate) async fn connect_ws_detailed(
    http_url: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    WsConnectError,
> {
    let ws_url = if let Some(rest) = http_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if let Some(rest) = http_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if http_url.starts_with("ws://") || http_url.starts_with("wss://") {
        http_url.to_string()
    } else {
        return Err(WsConnectError::Other(anyhow::anyhow!(
            "unsupported URL scheme: {http_url}"
        )));
    };

    match tokio_tungstenite::connect_async(ws_url).await {
        Ok((ws, _resp)) => Ok(ws),
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
            Err(WsConnectError::HttpStatus(resp.status().as_u16()))
        }
        Err(e) => Err(WsConnectError::Other(anyhow::Error::new(e))),
    }
}

pub(crate) async fn connect_ws(
    http_url: &str,
) -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let ws_url = if let Some(rest) = http_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if let Some(rest) = http_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if http_url.starts_with("ws://") || http_url.starts_with("wss://") {
        http_url.to_string()
    } else {
        anyhow::bail!("unsupported URL scheme: {http_url}");
    };

    let (ws, _resp) = tokio_tungstenite::connect_async(ws_url).await?;
    Ok(ws)
}

pub(crate) enum WsParsed {
    Patch(json_patch::Patch),
    Finished,
    Ignored,
    Error(String),
}

pub(crate) fn parse_ws_message(text: &str) -> WsParsed {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return WsParsed::Error(e.to_string()),
    };

    if value.get("finished").and_then(|v| v.as_bool()) == Some(true) {
        return WsParsed::Finished;
    }

    let patch_value = match value.get("JsonPatch") {
        Some(v) => v.clone(),
        None => return WsParsed::Ignored,
    };

    let patch: json_patch::Patch = match serde_json::from_value(patch_value) {
        Ok(p) => p,
        Err(e) => return WsParsed::Error(e.to_string()),
    };

    WsParsed::Patch(patch)
}
