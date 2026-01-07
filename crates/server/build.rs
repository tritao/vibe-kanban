use std::{fs, path::Path};

fn main() {
    dotenv::dotenv().ok();

    // Embed git commit hash in the binary for easier debugging (e.g. when multiple backends run).
    // This may be "unknown" when building from a source archive without git metadata.
    let git_commit = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=VIBE_GIT_COMMIT={git_commit}");

    if let Ok(api_key) = std::env::var("POSTHOG_API_KEY") {
        println!("cargo:rustc-env=POSTHOG_API_KEY={}", api_key);
    }
    if let Ok(api_endpoint) = std::env::var("POSTHOG_API_ENDPOINT") {
        println!("cargo:rustc-env=POSTHOG_API_ENDPOINT={}", api_endpoint);
    }
    if let Ok(vk_shared_api_base) = std::env::var("VK_SHARED_API_BASE") {
        println!("cargo:rustc-env=VK_SHARED_API_BASE={}", vk_shared_api_base);
    }

    // Create frontend/dist directory if it doesn't exist
    let dist_path = Path::new("../../frontend/dist");
    if !dist_path.exists() {
        println!("cargo:warning=Creating dummy frontend/dist directory for compilation");
        fs::create_dir_all(dist_path).unwrap();

        // Create a dummy index.html
        let dummy_html = r#"<!DOCTYPE html>
<html><head><title>Build frontend first</title></head>
<body><h1>Please build the frontend</h1></body></html>"#;

        fs::write(dist_path.join("index.html"), dummy_html).unwrap();
    }
}
