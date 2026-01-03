use std::process::Command;

pub(crate) fn open_url(url: &str) -> anyhow::Result<()> {
    let url = url.trim();
    if url.is_empty() {
        anyhow::bail!("missing URL");
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open").arg(url).status()?;
        if !status.success() {
            anyhow::bail!("failed to run `open`");
        }
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        // `start` is a `cmd.exe` builtin; empty title is required when the URL contains `:`.
        let status = Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()?;
        if !status.success() {
            anyhow::bail!("failed to run `cmd /C start`");
        }
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let status = Command::new("xdg-open").arg(url).status()?;
        if !status.success() {
            anyhow::bail!("failed to run `xdg-open`");
        }
        return Ok(());
    }
}

