use std::io::{self, Write};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use crossterm::{QueueableCommand, style::Print};

pub(crate) fn copy_to_clipboard_osc52(text: &str) -> anyhow::Result<()> {
    // Many terminals impose payload limits; keep this conservative.
    const MAX_BYTES: usize = 64 * 1024;
    let bytes = text.as_bytes();
    let bytes = if bytes.len() > MAX_BYTES {
        &bytes[..MAX_BYTES]
    } else {
        bytes
    };

    let b64 = STANDARD.encode(bytes);
    let seq = format!("\x1b]52;c;{b64}\x07");

    let mut out = io::stdout();
    out.queue(Print(seq))?;
    out.flush()?;
    Ok(())
}
