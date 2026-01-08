use uuid::Uuid;

use crate::net::api_client::{decode_api_response, http_client, url};

#[derive(Debug, serde::Deserialize)]
struct ProjectIdDto {
    id: Uuid,
}

#[derive(Debug, serde::Deserialize)]
struct ProjectRepoPathDto {
    #[serde(alias = "git_repo_path", alias = "path")]
    path: String,
}

pub(crate) async fn create_project_http(
    base_url: &str,
    name: &str,
    repo_path: &str,
    display_name: &str,
) -> anyhow::Result<Uuid> {
    let client = http_client()?;

    let endpoint = url(base_url, "/api/projects");
    let body = serde_json::json!({
        "name": name,
        "repositories": [
            {
                "display_name": display_name,
                "git_repo_path": repo_path,
            }
        ],
    });

    let resp = client.post(endpoint).json(&body).send().await?;
    let api = decode_api_response::<ProjectIdDto>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected project create");
    }
    Ok(api
        .into_data()
        .ok_or_else(|| anyhow::anyhow!("missing project in create response"))?
        .id)
}

pub(crate) async fn add_project_repository_http(
    base_url: &str,
    project_id: Uuid,
    repo_path: &str,
    display_name: &str,
) -> anyhow::Result<()> {
    let client = http_client()?;

    let endpoint = url(
        base_url,
        &format!("/api/projects/{project_id}/repositories"),
    );
    let body = serde_json::json!({
        "display_name": display_name,
        "git_repo_path": repo_path,
    });

    let resp = client.post(endpoint).json(&body).send().await?;
    let api = decode_api_response::<()>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected add repository");
    }
    Ok(())
}

pub(crate) async fn find_project_for_repo_path_http(
    base_url: &str,
    repo_path: &str,
) -> anyhow::Result<Option<Uuid>> {
    let client = http_client()?;

    let target = std::fs::canonicalize(repo_path)
        .unwrap_or_else(|_| std::path::PathBuf::from(repo_path))
        .to_string_lossy()
        .to_string();

    let endpoint = url(base_url, "/api/projects");
    let resp = client.get(endpoint).send().await?;
    let api = decode_api_response::<Vec<ProjectIdDto>>(resp).await?;
    if !api.is_success() {
        anyhow::bail!("backend rejected projects request");
    }
    let projects = api.into_data().unwrap_or_default();

    for p in projects {
        let project_id = p.id;

        let endpoint = url(
            base_url,
            &format!("/api/projects/{project_id}/repositories"),
        );
        let resp = client.get(endpoint).send().await?;
        let api = decode_api_response::<Vec<ProjectRepoPathDto>>(resp).await?;
        if !api.is_success() {
            continue;
        }
        let repos = api.into_data().unwrap_or_default();
        for r in repos {
            let path_str = r.path;
            if !path_str.trim().is_empty() {
                let p2 = std::fs::canonicalize(&path_str)
                    .unwrap_or_else(|_| std::path::PathBuf::from(path_str))
                    .to_string_lossy()
                    .to_string();
                if p2 == target {
                    return Ok(Some(project_id));
                }
            }
        }
    }

    Ok(None)
}
