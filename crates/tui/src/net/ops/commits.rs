use uuid::Uuid;

use crate::net::{
    api_client::{decode_json_response, http_client, url},
    ops::wire::ApiResponseWire,
};

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CommitEntryWire {
    pub(crate) oid: String,
    pub(crate) short_oid: String,
    pub(crate) unix_ts: i64,
    pub(crate) subject: String,
}

pub(crate) async fn commit_list_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
    limit: Option<usize>,
    offset: Option<usize>,
) -> anyhow::Result<Vec<crate::state::CommitEntry>> {
    let client = http_client()?;

    let mut endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/commits?repo_id={repo_id}"),
    );
    if let Some(limit) = limit {
        endpoint.push_str(&format!("&limit={limit}"));
    }
    if let Some(offset) = offset {
        endpoint.push_str(&format!("&offset={offset}"));
    }

    let resp = client.get(endpoint).send().await?;
    let api = decode_json_response::<ApiResponseWire<Vec<CommitEntryWire>>>(resp).await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected commit list request")
        );
    }
    let commits = api.data.unwrap_or_default();
    Ok(commits
        .into_iter()
        .map(|c| crate::state::CommitEntry {
            oid: c.oid,
            short_oid: c.short_oid,
            unix_ts: c.unix_ts,
            subject: c.subject,
        })
        .collect())
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CommitShowWire {
    pub(crate) text: String,
}

pub(crate) async fn commit_show_http(
    base_url: &str,
    attempt_id: Uuid,
    repo_id: Uuid,
    oid: &str,
) -> anyhow::Result<String> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/task-attempts/{attempt_id}/commits/{oid}?repo_id={repo_id}"),
    );
    let resp = client.get(endpoint).send().await?;
    let api = decode_json_response::<ApiResponseWire<CommitShowWire>>(resp).await?;
    if !api.success {
        anyhow::bail!(
            "{}",
            api.message
                .as_deref()
                .unwrap_or("backend rejected commit show request")
        );
    }
    Ok(api
        .data
        .ok_or_else(|| anyhow::anyhow!("missing commit show payload"))?
        .text)
}
