use std::sync::OnceLock;

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

use crate::{
    text::{display_width, truncate_spans_to_width, wrap_spans_hard},
};
use crate::state::DiffTheme;

fn syntect_syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn syntect_theme_set() -> &'static ThemeSet {
    static SET: OnceLock<ThemeSet> = OnceLock::new();
    SET.get_or_init(ThemeSet::load_defaults)
}

fn syntect_fallback_theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(Theme::default)
}

fn syntect_theme(theme: DiffTheme) -> &'static Theme {
    let ts = syntect_theme_set();
    ts.themes
        .get(theme.syntect_key())
        .or_else(|| ts.themes.get(DiffTheme::default().syntect_key()))
        .or_else(|| ts.themes.values().next())
        .unwrap_or_else(|| syntect_fallback_theme())
}

fn syntax_for_path<'a>(ps: &'a SyntaxSet, path: &str) -> &'a SyntaxReference {
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path);

    if file_name == "Dockerfile" {
        if let Some(s) = ps.find_syntax_by_name("Dockerfile") {
            return s;
        }
    }
    if file_name == "Makefile" {
        if let Some(s) = ps.find_syntax_by_name("Makefile") {
            return s;
        }
    }

    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|s| s.to_str());
    if let Some(ext) = ext {
        if let Some(syntax) = ps.find_syntax_by_extension(ext) {
            return syntax;
        }
        // Common aliases
        if ext == "rs" {
            if let Some(syntax) = ps.find_syntax_by_extension("rust") {
                return syntax;
            }
        }
        if ext == "yml" {
            if let Some(syntax) = ps.find_syntax_by_extension("yaml") {
                return syntax;
            }
        }
    }

    ps.find_syntax_plain_text()
}

fn syntect_style_to_ratatui(style: syntect::highlighting::Style) -> Style {
    let fg = style.foreground;
    Style::default().fg(Color::Rgb(fg.r, fg.g, fg.b))
}

fn color_to_rgb(c: Color) -> Option<(u8, u8, u8)> {
    match c {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        Color::Black => Some((0, 0, 0)),
        Color::Red => Some((205, 49, 49)),
        Color::Green => Some((13, 188, 121)),
        Color::Yellow => Some((229, 229, 16)),
        Color::Blue => Some((36, 114, 200)),
        Color::Magenta => Some((188, 63, 188)),
        Color::Cyan => Some((17, 168, 205)),
        Color::Gray => Some((204, 204, 204)),
        Color::DarkGray => Some((118, 118, 118)),
        Color::LightRed => Some((241, 76, 76)),
        Color::LightGreen => Some((35, 209, 139)),
        Color::LightYellow => Some((245, 245, 67)),
        Color::LightBlue => Some((59, 142, 234)),
        Color::LightMagenta => Some((214, 112, 214)),
        Color::LightCyan => Some((41, 184, 219)),
        Color::White => Some((255, 255, 255)),
        _ => None,
    }
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance(rgb: (u8, u8, u8)) -> f64 {
    let (r, g, b) = rgb;
    let r = srgb_to_linear(r as f64 / 255.0);
    let g = srgb_to_linear(g as f64 / 255.0);
    let b = srgb_to_linear(b as f64 / 255.0);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (l1, l2) = if la >= lb { (la, lb) } else { (lb, la) };
    (l1 + 0.05) / (l2 + 0.05)
}

fn best_contrast_bw(bg: (u8, u8, u8)) -> Color {
    let black = (0, 0, 0);
    let white = (255, 255, 255);
    if contrast_ratio(white, bg) >= contrast_ratio(black, bg) {
        Color::White
    } else {
        Color::Black
    }
}

pub(crate) fn highlight_unified_diff(
    file_path: &str,
    diff: &str,
    width: usize,
    theme: DiffTheme,
    wrap: bool,
) -> Vec<Line<'static>> {
    let ps = syntect_syntax_set();
    let syntect_theme = syntect_theme(theme);
    let syntax = syntax_for_path(ps, file_path);

    let mut old_hl = HighlightLines::new(syntax, syntect_theme);
    let mut new_hl = HighlightLines::new(syntax, syntect_theme);

    let mut out: Vec<Line<'static>> = vec![];
    for raw_line in diff.lines() {
        if raw_line.starts_with("--- ") || raw_line.starts_with("+++ ") {
            let spans = vec![Span::styled(
                raw_line.to_string(),
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            )];
            if wrap {
                for row in wrap_spans_hard(spans, width) {
                    out.push(Line::from(row));
                }
            } else {
                out.push(Line::from(truncate_spans_to_width(spans, width)));
            }
            continue;
        }

        if raw_line.starts_with("@@") {
            let spans = vec![Span::styled(
                raw_line.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )];
            if wrap {
                for row in wrap_spans_hard(spans, width) {
                    out.push(Line::from(row));
                }
            } else {
                out.push(Line::from(truncate_spans_to_width(spans, width)));
            }
            continue;
        }

        let (marker, rest) = raw_line.split_at(1.min(raw_line.len()));
        let marker_ch = marker.chars().next().unwrap_or(' ');
        let (gutter_style, marker_style) = match marker_ch {
            '+' => (
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
            '-' => (
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            ' ' => (
                Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                Style::default().fg(Color::Gray),
            ),
            _ => (
                Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                Style::default().fg(Color::Gray),
            ),
        };

        let mut spans: Vec<Span<'static>> = vec![
            Span::styled("▌".to_string(), gutter_style),
            Span::styled(marker.to_string(), marker_style),
        ];
        let rest_spans = match marker_ch {
            '+' => new_hl
                .highlight_line(rest, ps)
                .map(|ranges| {
                    ranges
                        .into_iter()
                        .map(|(s, t)| Span::styled(t.to_string(), syntect_style_to_ratatui(s)))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|_| vec![Span::raw(rest.to_string())]),
            '-' => old_hl
                .highlight_line(rest, ps)
                .map(|ranges| {
                    ranges
                        .into_iter()
                        .map(|(s, t)| Span::styled(t.to_string(), syntect_style_to_ratatui(s)))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|_| vec![Span::raw(rest.to_string())]),
            ' ' => {
                // Advance both sides for better multi-line state.
                let _ = new_hl.highlight_line(rest, ps);
                old_hl
                    .highlight_line(rest, ps)
                    .map(|ranges| {
                        ranges
                            .into_iter()
                            .map(|(s, t)| Span::styled(t.to_string(), syntect_style_to_ratatui(s)))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|_| vec![Span::raw(rest.to_string())])
            }
            _ => vec![Span::raw(rest.to_string())],
        };
        spans.extend(rest_spans);

        // Semi-transparent GitHub-like backgrounds for additions/removals:
        // - added: hsl(138 69% 45% / .4)
        // - removed: hsl(5 100% 69% / .4)
        //
        // Terminals don't support alpha backgrounds, so we approximate by blending the HSL color
        // against the theme's "likely" background (white for light themes; #1e1e1e for dark).
        let line_bg = {
            fn blend_2_5(fg: u8, bg: u8) -> u8 {
                // 0.4*fg + 0.6*bg == (2/5)*fg + (3/5)*bg
                let v = (fg as u16) * 2 + (bg as u16) * 3;
                ((v + 2) / 5) as u8
            }

            let (bg_r, bg_g, bg_b) = if theme.is_light() {
                (255u8, 255u8, 255u8)
            } else {
                (30u8, 30u8, 30u8)
            };

            // Precomputed from HSL:
            // - hsl(138 69% 45%) => rgb(36, 194, 83)
            // - hsl(5 100% 69%)  => rgb(255, 110, 97)
            if marker_ch == '+' {
                Some(Color::Rgb(
                    blend_2_5(36, bg_r),
                    blend_2_5(194, bg_g),
                    blend_2_5(83, bg_b),
                ))
            } else if marker_ch == '-' {
                Some(Color::Rgb(
                    blend_2_5(255, bg_r),
                    blend_2_5(110, bg_g),
                    blend_2_5(97, bg_b),
                ))
            } else {
                None
            }
        };

        if let Some(bg) = line_bg {
            for s in spans.iter_mut() {
                s.style = s.style.bg(bg);
            }
        }

        let apply_contrast = |spans: &mut [Span<'static>], bg: Color, skip: usize| {
            if let Some(bg_rgb) = color_to_rgb(bg) {
                const MIN_CONTRAST: f64 = 3.0;
                for span in spans.iter_mut().skip(skip) {
                    let Some(fg) = span.style.fg else {
                        continue;
                    };
                    let Some(fg_rgb) = color_to_rgb(fg) else {
                        continue;
                    };
                    if contrast_ratio(fg_rgb, bg_rgb) < MIN_CONTRAST {
                        span.style = span.style.fg(best_contrast_bw(bg_rgb));
                    }
                }
            }
        };

        let pad_bg = |spans: &mut Vec<Span<'static>>, bg: Color| {
            let used = spans
                .iter()
                .map(|s| display_width(s.content.as_ref()))
                .sum::<usize>();
            if used < width {
                spans.push(Span::styled(
                    " ".repeat(width - used),
                    Style::default().bg(bg),
                ));
            }
        };

        if wrap && width > 2 {
            let content_width = width.saturating_sub(2).max(1);
            let content = spans.split_off(2);
            let wrapped = wrap_spans_hard(content, content_width);

            for (i, chunk) in wrapped.into_iter().enumerate() {
                let mut line_spans: Vec<Span<'static>> = if i == 0 {
                    spans.clone()
                } else {
                    let mut p = vec![
                        Span::styled("▌".to_string(), gutter_style),
                        Span::styled(" ".to_string(), marker_style),
                    ];
                    if let Some(bg) = line_bg {
                        for s in p.iter_mut() {
                            s.style = s.style.bg(bg);
                        }
                    }
                    p
                };
                line_spans.extend(chunk);
                if let Some(bg) = line_bg {
                    apply_contrast(&mut line_spans, bg, 2);
                    pad_bg(&mut line_spans, bg);
                }
                out.push(Line::from(line_spans));
            }
        } else {
            let mut spans = truncate_spans_to_width(spans, width);
            if let Some(bg) = line_bg {
                apply_contrast(&mut spans, bg, 2);
                pad_bg(&mut spans, bg);
            }
            out.push(Line::from(spans));
        }
    }
    out
}
