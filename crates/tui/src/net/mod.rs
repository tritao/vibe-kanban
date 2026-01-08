use anyhow::Context;
use utils::port_file::read_port_file;

use crate::Args;

pub(crate) mod api_client;
pub(crate) mod ops;
pub(crate) mod streams;
pub(crate) mod tasks;

pub(crate) use streams::{
    diff_stream_task, exec_stream_task, logs_stream_task, projects_stream_task, tasks_stream_task,
};
pub(crate) use tasks::{load_attempts_task, load_info_task};

pub(crate) async fn resolve_backend_url(args: &Args) -> anyhow::Result<String> {
    if let Some(url) = args.backend_url.as_ref().filter(|s| !s.trim().is_empty()) {
        return Ok(url.trim_end_matches('/').to_string());
    }

    let host = args
        .host
        .clone()
        .or_else(|| std::env::var("HOST").ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let port = if let Some(p) = args.port {
        p
    } else if let Ok(port_str) = std::env::var("BACKEND_PORT").or_else(|_| std::env::var("PORT")) {
        port_str.parse::<u16>().context("invalid port value")?
    } else {
        match read_port_file("vibe-kanban").await {
            Ok(port) => port,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let port_path = std::env::temp_dir()
                    .join("vibe-kanban")
                    .join("vibe-kanban.port");
                return Err(anyhow::anyhow!(
                    "Could not find backend port. Start the backend (e.g. `pnpm run dev`), or pass `--backend-url http://127.0.0.1:PORT`, or set `BACKEND_PORT`.\nMissing port file: {}",
                    port_path.display()
                ));
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to read backend port file: {e} (set `BACKEND_PORT` or pass `--backend-url`)"
                ));
            }
        }
    };

    Ok(format!("http://{}:{}", host, port))
}
