use std::borrow::Cow;

pub(crate) fn sanitize_tui_text(s: &str) -> Cow<'_, str> {
    // Control chars (especially '\r') and ANSI escape sequences can cause cursor movement and
    // visual corruption when written to the terminal. Strip them before rendering.
    fn needs_sanitize(s: &str) -> bool {
        s.as_bytes()
            .iter()
            .any(|&b| b == b'\x1b' || b == b'\r' || (b < 0x20 && b != b'\n') || b == 0x7f)
    }

    if !needs_sanitize(s) {
        return Cow::Borrowed(s);
    }

    #[derive(Clone, Copy, Debug)]
    enum State {
        Text,
        Esc,
        Csi,
        Osc,
    }

    let mut out = String::with_capacity(s.len());
    let mut state = State::Text;
    let mut osc_esc = false;

    for ch in s.chars() {
        match state {
            State::Text => match ch {
                '\x1b' => state = State::Esc,
                '\r' => {
                    // Drop CR to avoid carriage-return overwrites.
                }
                '\n' => {
                    // Preserve newlines for Markdown parsing and multi-line rendering.
                    out.push('\n');
                }
                '\t' => {
                    // Expand tabs to spaces for consistent width handling.
                    out.push_str("    ");
                }
                c if c.is_control() => {
                    // Drop other control chars; they can corrupt layout.
                }
                _ => out.push(ch),
            },
            State::Esc => {
                // ESC [ ... (CSI) or ESC ] ... (OSC); otherwise drop and return to text.
                match ch {
                    '[' => state = State::Csi,
                    ']' => {
                        state = State::Osc;
                        osc_esc = false;
                    }
                    _ => state = State::Text,
                }
            }
            State::Csi => {
                // Consume until final byte in the CSI range (@..~).
                if ('@'..='~').contains(&ch) {
                    state = State::Text;
                }
            }
            State::Osc => {
                // Consume OSC until BEL or ST (ESC \).
                if osc_esc {
                    if ch == '\\' {
                        state = State::Text;
                    }
                    osc_esc = false;
                } else if ch == '\x07' {
                    state = State::Text;
                } else if ch == '\x1b' {
                    osc_esc = true;
                }
            }
        }
    }

    Cow::Owned(out)
}
