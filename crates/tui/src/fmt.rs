pub(crate) fn short_time(iso: &str) -> Option<&str> {
    // Best-effort extraction of "HH:MM:SS" from RFC3339 timestamps.
    // Example: "2026-01-02T09:31:00.123Z" -> "09:31:00"
    let t = iso.split('T').nth(1)?;
    let time = t.split(['.', 'Z', '+', '-']).next()?;
    if time.len() >= 8 {
        Some(&time[..8])
    } else {
        Some(time)
    }
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

pub(crate) fn op_failed(op: &str, e: impl std::fmt::Display) -> String {
    format!("{op} failed: {e}")
}
