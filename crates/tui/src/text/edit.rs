use unicode_width::UnicodeWidthChar;

pub(crate) fn clamp_cursor_to_boundary(s: &str, cursor: usize) -> usize {
    let cursor = cursor.min(s.len());
    if s.is_char_boundary(cursor) {
        return cursor;
    }
    // Move left until we find a valid boundary.
    let mut c = cursor;
    while c > 0 && !s.is_char_boundary(c) {
        c -= 1;
    }
    c
}

pub(crate) fn prev_cursor(s: &str, cursor: usize) -> usize {
    let cursor = clamp_cursor_to_boundary(s, cursor);
    if cursor == 0 {
        return 0;
    }
    s[..cursor]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

pub(crate) fn next_cursor(s: &str, cursor: usize) -> usize {
    let cursor = clamp_cursor_to_boundary(s, cursor);
    if cursor >= s.len() {
        return s.len();
    }
    let mut iter = s[cursor..].char_indices();
    let _ = iter.next(); // current char at 0
    match iter.next() {
        Some((i, _)) => cursor + i,
        None => s.len(),
    }
}

pub(crate) fn prev_word_cursor(s: &str, cursor: usize) -> usize {
    let mut cur = clamp_cursor_to_boundary(s, cursor);
    if cur == 0 {
        return 0;
    }

    while cur > 0 {
        let prev = prev_cursor(s, cur);
        let ch = s.get(prev..cur).and_then(|t| t.chars().next());
        if ch.is_some_and(|c| c.is_whitespace()) {
            cur = prev;
        } else {
            break;
        }
    }

    while cur > 0 {
        let prev = prev_cursor(s, cur);
        let ch = s.get(prev..cur).and_then(|t| t.chars().next());
        if ch.is_some_and(|c| c.is_whitespace()) {
            break;
        }
        cur = prev;
    }

    cur
}

pub(crate) fn next_word_cursor(s: &str, cursor: usize) -> usize {
    let mut cur = clamp_cursor_to_boundary(s, cursor);
    let len = s.len();
    if cur >= len {
        return len;
    }

    while cur < len {
        let next = next_cursor(s, cur);
        let ch = s.get(cur..next).and_then(|t| t.chars().next());
        if ch.is_some_and(|c| c.is_whitespace()) {
            cur = next;
        } else {
            break;
        }
    }

    while cur < len {
        let next = next_cursor(s, cur);
        let ch = s.get(cur..next).and_then(|t| t.chars().next());
        if ch.is_some_and(|c| c.is_whitespace()) {
            break;
        }
        cur = next;
    }

    cur
}

pub(crate) fn line_ranges(s: &str) -> Vec<(usize, usize)> {
    if s.is_empty() {
        return vec![(0, 0)];
    }
    let mut ranges: Vec<(usize, usize)> = vec![];
    let mut start = 0usize;
    for (i, ch) in s.char_indices() {
        if ch == '\n' {
            ranges.push((start, i));
            start = i + 1;
        }
    }
    ranges.push((start, s.len()));
    ranges
}

pub(crate) fn cursor_line_index(ranges: &[(usize, usize)], cursor: usize) -> usize {
    for (idx, (start, end)) in ranges.iter().enumerate() {
        if cursor < *start {
            return idx.saturating_sub(1);
        }
        if cursor <= *end {
            return idx;
        }
    }
    ranges.len().saturating_sub(1)
}

pub(crate) fn cursor_col_in_line(s: &str, line_start: usize, cursor: usize) -> usize {
    let cursor = cursor.max(line_start);
    let slice = s.get(line_start..cursor).unwrap_or("");
    unicode_width::UnicodeWidthStr::width(slice)
}

pub(crate) fn byte_index_at_display_col(line: &str, target_col: usize) -> usize {
    let mut col = 0usize;
    let mut last = 0usize;
    for (i, ch) in line.char_indices() {
        if col >= target_col {
            return last;
        }
        last = i;
        col = col.saturating_add(UnicodeWidthChar::width(ch).unwrap_or(0).max(1));
    }
    line.len()
}

pub(crate) fn move_cursor_vertically(
    s: &str,
    cursor: usize,
    delta_lines: i32,
    goal_col: Option<usize>,
) -> (usize, Option<usize>) {
    let cursor = clamp_cursor_to_boundary(s, cursor);
    let ranges = line_ranges(s);
    let cur_line = cursor_line_index(&ranges, cursor);
    let cur_line = cur_line.min(ranges.len().saturating_sub(1));

    let (cur_start, cur_end) = ranges[cur_line];
    let cur_col = goal_col.unwrap_or_else(|| cursor_col_in_line(s, cur_start, cursor.min(cur_end)));
    let target_line_i32 = cur_line as i32 + delta_lines;
    let target_line = if target_line_i32 < 0 {
        0usize
    } else {
        (target_line_i32 as usize).min(ranges.len().saturating_sub(1))
    };
    let (t_start, t_end) = ranges[target_line];
    let line_str = s.get(t_start..t_end).unwrap_or("");
    let within = byte_index_at_display_col(line_str, cur_col);
    (
        t_start + within.min(t_end.saturating_sub(t_start)),
        Some(cur_col),
    )
}
