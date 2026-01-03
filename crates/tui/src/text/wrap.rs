use ratatui::{
    style::Style,
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

pub(crate) fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub(crate) fn truncate_to_width(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if display_width(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars() {
        if display_width(&out) >= max.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

pub(crate) fn split_by_width(s: &str, max: usize) -> (String, String) {
    if max == 0 {
        return (String::new(), s.to_string());
    }
    let mut chunk = String::new();
    let mut last_byte = 0usize;
    for (i, ch) in s.char_indices() {
        let next = format!("{chunk}{ch}");
        if display_width(&next) > max {
            break;
        }
        chunk.push(ch);
        last_byte = i + ch.len_utf8();
    }
    let rest = s.get(last_byte..).unwrap_or("").to_string();
    (chunk, rest)
}

pub(crate) fn push_span_merged(spans: &mut Vec<Span<'static>>, text: String, style: Style) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.style == style
    {
        last.content.to_mut().push_str(&text);
        return;
    }
    spans.push(Span::styled(text, style));
}

pub(crate) fn line_display_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|s| display_width(s.content.as_ref()))
        .sum()
}

pub(crate) fn split_token_prefer_separators(s: &str, max: usize) -> (String, String) {
    if max == 0 {
        return (String::new(), s.to_string());
    }
    if display_width(s) <= max {
        return (s.to_string(), String::new());
    }

    let mut chunk = String::new();
    let mut last_soft_break: Option<usize> = None;
    for (i, ch) in s.char_indices() {
        let next = format!("{chunk}{ch}");
        if display_width(&next) > max {
            break;
        }
        chunk.push(ch);
        let end = i + ch.len_utf8();
        if matches!(
            ch,
            '/' | '-' | '_' | '.' | ':' | '@' | '?' | '&' | '=' | '#'
        ) {
            last_soft_break = Some(end);
        }
    }

    let cut = last_soft_break.unwrap_or_else(|| {
        let (c, _) = split_by_width(s, max);
        c.len()
    });

    let mut left = s.get(..cut).unwrap_or("").to_string();
    let mut right = s.get(cut..).unwrap_or("").to_string();
    // Trim spaces around the break when breaking at a separator boundary.
    left = left.trim_end().to_string();
    right = right.trim_start().to_string();
    (left, right)
}

pub(crate) fn wrap_line_wordwise(line: &Line<'static>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    if line.spans.is_empty() || line_display_width(line) <= width {
        return vec![line.clone()];
    }

    #[derive(Clone)]
    struct Tok {
        text: String,
        style: Style,
        is_ws: bool,
    }

    let mut tokens: Vec<Tok> = vec![];
    for span in &line.spans {
        let style = span.style;
        let text = span.content.as_ref();
        if text.is_empty() {
            continue;
        }
        let mut cur = String::new();
        let mut cur_ws: Option<bool> = None;
        for ch in text.chars() {
            let is_ws = ch.is_whitespace();
            if cur_ws == Some(is_ws) || cur_ws.is_none() {
                cur.push(ch);
                cur_ws = Some(is_ws);
            } else {
                tokens.push(Tok {
                    text: cur.clone(),
                    style,
                    is_ws: cur_ws.unwrap_or(false),
                });
                cur.clear();
                cur.push(ch);
                cur_ws = Some(is_ws);
            }
        }
        if !cur.is_empty() {
            tokens.push(Tok {
                text: cur,
                style,
                is_ws: cur_ws.unwrap_or(false),
            });
        }
    }

    let mut out: Vec<Line<'static>> = vec![];
    let mut cur_spans: Vec<Span<'static>> = vec![];
    let mut cur_w: usize = 0;

    let mut i = 0usize;
    while i < tokens.len() {
        let tok = tokens[i].clone();
        if tok.is_ws {
            // Avoid starting wrapped lines with incidental whitespace.
            if cur_spans.is_empty() {
                // Keep indentation (2+ spaces) if the original line started with it.
                if tok.text.chars().all(|c| c == ' ') && tok.text.len() >= 2 {
                    let w = display_width(&tok.text);
                    if w <= width {
                        push_span_merged(&mut cur_spans, tok.text, tok.style);
                        cur_w += w;
                    }
                }
                i += 1;
                continue;
            }

            // Normalize whitespace between words to a single space for wrapping.
            let space = " ".to_string();
            let w = 1usize;
            if cur_w + w > width {
                out.push(Line::from(cur_spans.clone()));
                cur_spans.clear();
                cur_w = 0;
                i += 1;
                continue;
            }
            push_span_merged(&mut cur_spans, space, tok.style);
            cur_w += w;
            i += 1;
            continue;
        }

        // Non-whitespace token
        let mut text = tok.text;
        let style = tok.style;
        loop {
            let w = display_width(&text);
            if cur_w + w <= width {
                push_span_merged(&mut cur_spans, text, style);
                cur_w += w;
                break;
            }

            if !cur_spans.is_empty() {
                // Word-wrap: move token to next line.
                out.push(Line::from(cur_spans.clone()));
                cur_spans.clear();
                cur_w = 0;
                continue;
            }

            // Token longer than the full line: split on soft separators first.
            let (chunk, rest) = split_token_prefer_separators(&text, width);
            if chunk.is_empty() {
                let (c, r) = split_by_width(&text, width);
                if c.is_empty() {
                    break;
                }
                push_span_merged(&mut cur_spans, c, style);
                out.push(Line::from(cur_spans.clone()));
                cur_spans.clear();
                cur_w = 0;
                text = r;
                if text.is_empty() {
                    break;
                }
                continue;
            }
            push_span_merged(&mut cur_spans, chunk, style);
            out.push(Line::from(cur_spans.clone()));
            cur_spans.clear();
            cur_w = 0;
            text = rest;
            if text.is_empty() {
                break;
            }
        }
        i += 1;
    }

    if !cur_spans.is_empty() {
        out.push(Line::from(cur_spans));
    }

    out
}

pub(crate) fn truncate_spans_to_width(mut spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    if width == 0 {
        return vec![];
    }

    let mut out: Vec<Span<'static>> = vec![];
    let mut used = 0usize;
    for span in spans.drain(..) {
        let w = display_width(span.content.as_ref());
        if used + w <= width {
            used += w;
            out.push(span);
            continue;
        }

        let remaining = width.saturating_sub(used);
        if remaining == 0 {
            break;
        }

        let truncated = truncate_to_width(span.content.as_ref(), remaining);
        out.push(Span::styled(truncated, span.style));
        break;
    }
    out
}

fn split_spans_by_width(
    spans: &[Span<'static>],
    max: usize,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    if max == 0 || spans.is_empty() {
        return (vec![], spans.to_vec());
    }

    let mut left: Vec<Span<'static>> = vec![];
    let mut remaining = max;
    for (i, span) in spans.iter().enumerate() {
        let w = display_width(span.content.as_ref());
        if w <= remaining {
            left.push(span.clone());
            remaining -= w;
            if remaining == 0 {
                return (left, spans[i + 1..].to_vec());
            }
            continue;
        }

        if remaining == 0 {
            return (left, spans[i..].to_vec());
        }

        let (chunk, rest) = split_by_width(span.content.as_ref(), remaining);
        if !chunk.is_empty() {
            left.push(Span::styled(chunk, span.style));
        }
        let mut right: Vec<Span<'static>> = vec![];
        if !rest.is_empty() {
            right.push(Span::styled(rest, span.style));
        }
        right.extend_from_slice(&spans[i + 1..]);
        return (left, right);
    }

    (left, vec![])
}

pub(crate) fn wrap_spans_hard(mut spans: Vec<Span<'static>>, width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let mut out: Vec<Vec<Span<'static>>> = vec![];
    while !spans.is_empty() {
        let total: usize = spans
            .iter()
            .map(|s| display_width(s.content.as_ref()))
            .sum();
        if total <= width {
            out.push(spans);
            break;
        }

        let (left, right) = split_spans_by_width(&spans, width);
        if left.is_empty() {
            // Ensure progress even with extremely small widths / wide chars.
            let first = spans.remove(0);
            out.push(vec![first]);
            continue;
        }
        out.push(left);
        spans = right;
    }
    out
}

