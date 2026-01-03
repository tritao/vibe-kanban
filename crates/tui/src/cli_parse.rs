pub(crate) fn tokenize_command_line(s: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = vec![];
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    let mut quote: Option<char> = None;

    while let Some(ch) = chars.next() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                    continue;
                }
                if ch == '\\' && q == '"' {
                    if let Some(next) = chars.next() {
                        cur.push(next);
                    }
                    continue;
                }
                cur.push(ch);
            }
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                '\\' => {
                    if let Some(next) = chars.next() {
                        cur.push(next);
                    }
                }
                c if c.is_whitespace() => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    while matches!(chars.peek(), Some(p) if p.is_whitespace()) {
                        chars.next();
                    }
                }
                _ => cur.push(ch),
            },
        }
    }

    if quote.is_some() {
        return Err("unterminated quote".to_string());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_whitespace() {
        assert_eq!(
            tokenize_command_line("a   b\tc\n d").unwrap(),
            vec!["a", "b", "c", "d"]
        );
    }

    #[test]
    fn tokenizes_quotes_and_escapes() {
        assert_eq!(
            tokenize_command_line("cmd --title \"hello world\" --x 'y z'").unwrap(),
            vec!["cmd", "--title", "hello world", "--x", "y z"]
        );
        assert_eq!(
            tokenize_command_line("cmd \"a\\\\\\\"b\"").unwrap(),
            vec!["cmd", "a\\\"b"]
        );
    }

    #[test]
    fn errors_on_unterminated_quote() {
        assert!(tokenize_command_line("cmd 'unterminated").is_err());
    }
}
