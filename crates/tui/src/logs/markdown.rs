use pulldown_cmark::{
    CodeBlockKind, Event as MdEvent, Options as MdOptions, Parser as MdParser, Tag as MdTag,
    TagEnd as MdTagEnd,
};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::text::{
    display_width, push_span_merged, sanitize_tui_text, split_by_width, truncate_to_width,
};

#[derive(Debug, Clone)]
enum MdToken {
    Text(String, Style),
    Newline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MdSoftBreakMode {
    Space,
    Newline,
}

fn wrap_md_tokens(
    tokens: &[MdToken],
    width: usize,
    prefix_first: &str,
    prefix_next: &str,
    prefix_style: Style,
) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut out: Vec<Line<'static>> = vec![];

    let mut cur_spans: Vec<Span<'static>> = vec![];
    let mut cur_w: usize = 0;
    let mut cur_prefix_w: usize = 0;

    let start_line = |spans: &mut Vec<Span<'static>>,
                      cur_w: &mut usize,
                      cur_prefix_w: &mut usize,
                      first: bool| {
        spans.clear();
        let prefix = if first { prefix_first } else { prefix_next };
        if !prefix.is_empty() {
            spans.push(Span::styled(prefix.to_string(), prefix_style));
            *cur_prefix_w = display_width(prefix);
            *cur_w = *cur_prefix_w;
        } else {
            *cur_prefix_w = 0;
            *cur_w = 0;
        }
    };

    start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, true);

    for token in tokens.iter().cloned() {
        match token {
            MdToken::Newline => {
                out.push(Line::from(cur_spans.clone()));
                start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, false);
            }
            MdToken::Text(mut text, style) => {
                if text == " " && cur_w == cur_prefix_w {
                    continue;
                }
                loop {
                    let available = width.saturating_sub(cur_w);
                    if available == 0 {
                        out.push(Line::from(cur_spans.clone()));
                        start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, false);
                        continue;
                    }

                    let w = display_width(&text);
                    if w <= available {
                        push_span_merged(&mut cur_spans, text, style);
                        cur_w += w;
                        break;
                    }

                    // Prefer word wrapping: if this is a non-space token and we're not at the
                    // beginning of the line, move it to the next line instead of splitting it.
                    if text != " " && cur_w > cur_prefix_w {
                        out.push(Line::from(cur_spans.clone()));
                        start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, false);
                        continue;
                    }

                    // If the token is too long even at the line start, hard-split by width.
                    let (chunk, rest) = split_by_width(&text, available);
                    if chunk.is_empty() {
                        // Should be rare; avoid infinite loops.
                        out.push(Line::from(cur_spans.clone()));
                        start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, false);
                        continue;
                    }
                    let chunk_w = display_width(&chunk);
                    push_span_merged(&mut cur_spans, chunk, style);
                    cur_w += chunk_w;
                    out.push(Line::from(cur_spans.clone()));
                    start_line(&mut cur_spans, &mut cur_w, &mut cur_prefix_w, false);
                    text = rest;
                    if text.is_empty() {
                        break;
                    }
                }
            }
        }
    }

    if !cur_spans.is_empty() {
        out.push(Line::from(cur_spans));
    }

    // Trim trailing empty lines.
    while out.last().is_some_and(|l| l.spans.is_empty()) {
        out.pop();
    }

    out
}

pub(crate) fn render_markdown(
    md: &str,
    width: usize,
    softbreak_mode: MdSoftBreakMode,
) -> Vec<Line<'static>> {
    let md = sanitize_tui_text(md);
    let width = width.max(1);

    let mut options = MdOptions::empty();
    options.insert(MdOptions::ENABLE_STRIKETHROUGH);
    options.insert(MdOptions::ENABLE_TABLES);
    options.insert(MdOptions::ENABLE_TASKLISTS);

    let parser = MdParser::new_ext(md.as_ref(), options);

    #[derive(Debug, Clone, Copy)]
    struct ListCtx {
        ordered: bool,
        next_number: usize,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum BlockKind {
        Paragraph,
        Heading,
        Item,
    }

    struct Block {
        kind: BlockKind,
        tokens: Vec<MdToken>,
        prefix_first: String,
        prefix_next: String,
        prefix_style: Style,
        trailing_blank_line: bool,
    }

    let mut out: Vec<Line<'static>> = vec![];
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut quote_depth: usize = 0;
    let mut lists: Vec<ListCtx> = vec![];
    let mut block: Option<Block> = None;

    let mut in_code_block = false;
    let mut code_buf = String::new();

    let prefix_style = Style::default().fg(crate::ui::palette::log_markdown_prefix_fg());

    fn base_prefix(quote_depth: usize, list_depth: usize) -> String {
        let mut p = String::new();
        if quote_depth > 0 {
            p.push_str(&"│ ".repeat(quote_depth));
        }
        if list_depth > 1 {
            p.push_str(&"  ".repeat(list_depth - 1));
        }
        p
    }

    let push_word = |block: &mut Block, word: &str, style: Style| {
        if let Some(MdToken::Text(prev, _)) = block.tokens.last() {
            if !prev.is_empty() && !prev.ends_with(' ') {
                block.tokens.push(MdToken::Text(" ".to_string(), style));
            }
        }
        block.tokens.push(MdToken::Text(word.to_string(), style));
    };

    let push_text = |block: &mut Block, text: &str, style: Style| {
        // pulldown-cmark generally emits explicit line breaks as SoftBreak/HardBreak events,
        // but some inputs can still contain literal '\n' inside Text events. Preserve those.
        let mut first_line = true;
        for line in text.split('\n') {
            if !first_line {
                block.tokens.push(MdToken::Newline);
            }
            first_line = false;
            for word in line.split_whitespace() {
                push_word(block, word, style);
            }
        }
    };

    let flush_block = |out: &mut Vec<Line<'static>>, block: &mut Option<Block>| {
        let Some(b) = block.take() else {
            return;
        };
        if b.tokens.is_empty() {
            return;
        }
        let lines = wrap_md_tokens(
            &b.tokens,
            width,
            &b.prefix_first,
            &b.prefix_next,
            b.prefix_style,
        );
        out.extend(lines);
        if b.trailing_blank_line {
            out.push(Line::from(""));
        }
    };

    for event in parser {
        if in_code_block {
            match event {
                MdEvent::End(MdTagEnd::CodeBlock) => {
                    let code_style =
                        Style::default().bg(crate::ui::palette::log_markdown_code_bg());
                    for l in code_buf.lines() {
                        out.push(Line::from(Span::styled(
                            truncate_to_width(l, width),
                            code_style,
                        )));
                    }
                    code_buf.clear();
                    in_code_block = false;
                    out.push(Line::from(""));
                }
                MdEvent::Text(t) | MdEvent::Code(t) => code_buf.push_str(&t),
                MdEvent::SoftBreak | MdEvent::HardBreak => code_buf.push('\n'),
                _ => {}
            }
            continue;
        }

        match event {
            MdEvent::Start(MdTag::Paragraph) => {
                if block.as_ref().is_some_and(|b| b.kind == BlockKind::Item) {
                    continue;
                }
                flush_block(&mut out, &mut block);
                let base = base_prefix(quote_depth, lists.len());
                block = Some(Block {
                    kind: BlockKind::Paragraph,
                    tokens: vec![],
                    prefix_first: base.clone(),
                    prefix_next: base,
                    prefix_style,
                    trailing_blank_line: true,
                });
            }
            MdEvent::End(MdTagEnd::Paragraph) => {
                if block.as_ref().is_some_and(|b| b.kind == BlockKind::Item) {
                    continue;
                }
                flush_block(&mut out, &mut block);
            }
            MdEvent::Start(MdTag::Heading { .. }) => {
                flush_block(&mut out, &mut block);
                let base = base_prefix(quote_depth, lists.len());
                block = Some(Block {
                    kind: BlockKind::Heading,
                    tokens: vec![],
                    prefix_first: base.clone(),
                    prefix_next: base,
                    prefix_style,
                    trailing_blank_line: true,
                });
                let h_style = Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(crate::ui::palette::log_markdown_heading_fg());
                style_stack.push(h_style);
            }
            MdEvent::End(MdTagEnd::Heading(_)) => {
                let _ = style_stack.pop();
                flush_block(&mut out, &mut block);
            }
            MdEvent::Start(MdTag::BlockQuote) => {
                quote_depth += 1;
            }
            MdEvent::End(MdTagEnd::BlockQuote) => {
                quote_depth = quote_depth.saturating_sub(1);
            }
            MdEvent::Start(MdTag::List(start)) => {
                let ordered = start.is_some();
                let next_number = start.unwrap_or(1) as usize;
                lists.push(ListCtx {
                    ordered,
                    next_number,
                });
            }
            MdEvent::End(MdTagEnd::List(_)) => {
                let _ = lists.pop();
                // A list boundary is a decent place to add separation.
                out.push(Line::from(""));
            }
            MdEvent::Start(MdTag::Item) => {
                flush_block(&mut out, &mut block);
                let base = base_prefix(quote_depth, lists.len());
                let bullet = if let Some(list) = lists.last_mut() {
                    if list.ordered {
                        let b = format!("{}. ", list.next_number);
                        list.next_number += 1;
                        b
                    } else {
                        "- ".to_string()
                    }
                } else {
                    "- ".to_string()
                };
                let cont = " ".repeat(display_width(&bullet));
                block = Some(Block {
                    kind: BlockKind::Item,
                    tokens: vec![],
                    prefix_first: format!("{base}{bullet}"),
                    prefix_next: format!("{base}{cont}"),
                    prefix_style,
                    trailing_blank_line: false,
                });
            }
            MdEvent::End(MdTagEnd::Item) => {
                flush_block(&mut out, &mut block);
            }
            MdEvent::Start(MdTag::CodeBlock(CodeBlockKind::Fenced(_)))
            | MdEvent::Start(MdTag::CodeBlock(CodeBlockKind::Indented)) => {
                flush_block(&mut out, &mut block);
                in_code_block = true;
                code_buf.clear();
            }
            MdEvent::Start(MdTag::Emphasis) => {
                let next = style_stack
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .add_modifier(Modifier::ITALIC);
                style_stack.push(next);
            }
            MdEvent::End(MdTagEnd::Emphasis) => {
                let _ = style_stack.pop();
            }
            MdEvent::Start(MdTag::Strong) => {
                let next = style_stack
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .add_modifier(Modifier::BOLD);
                style_stack.push(next);
            }
            MdEvent::End(MdTagEnd::Strong) => {
                let _ = style_stack.pop();
            }
            MdEvent::Start(MdTag::Link { .. }) => {
                let next = style_stack
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .add_modifier(Modifier::UNDERLINED);
                style_stack.push(next);
            }
            MdEvent::End(MdTagEnd::Link) => {
                let _ = style_stack.pop();
            }
            MdEvent::Code(t) => {
                let code_style = style_stack
                    .last()
                    .copied()
                    .unwrap_or_default()
                    .fg(crate::ui::palette::log_markdown_inline_code_fg());
                if block.is_none() {
                    let base = base_prefix(quote_depth, lists.len());
                    block = Some(Block {
                        kind: BlockKind::Paragraph,
                        tokens: vec![],
                        prefix_first: base.clone(),
                        prefix_next: base,
                        prefix_style,
                        trailing_blank_line: true,
                    });
                }
                if let Some(b) = block.as_mut() {
                    if let Some(MdToken::Text(prev, _)) = b.tokens.last() {
                        if !prev.is_empty() && !prev.ends_with(' ') {
                            b.tokens
                                .push(MdToken::Text(" ".to_string(), Style::default()));
                        }
                    }
                    // Treat inline code like normal text for wrapping purposes (but keep style).
                    for word in t.split_whitespace() {
                        push_word(b, word, code_style);
                    }
                }
            }
            MdEvent::Text(t) => {
                let style = style_stack.last().copied().unwrap_or_default();
                if block.is_none() {
                    let base = base_prefix(quote_depth, lists.len());
                    block = Some(Block {
                        kind: BlockKind::Paragraph,
                        tokens: vec![],
                        prefix_first: base.clone(),
                        prefix_next: base,
                        prefix_style,
                        trailing_blank_line: true,
                    });
                }
                if let Some(b) = block.as_mut() {
                    push_text(b, &t, style);
                }
            }
            MdEvent::SoftBreak => match softbreak_mode {
                MdSoftBreakMode::Space => {
                    if let Some(b) = block.as_mut() {
                        b.tokens
                            .push(MdToken::Text(" ".to_string(), Style::default()));
                    }
                }
                MdSoftBreakMode::Newline => {
                    if let Some(b) = block.as_mut() {
                        b.tokens.push(MdToken::Newline);
                    }
                }
            },
            MdEvent::HardBreak => {
                if let Some(b) = block.as_mut() {
                    b.tokens.push(MdToken::Newline);
                }
            }
            _ => {}
        }
    }

    flush_block(&mut out, &mut block);

    while out.last().is_some_and(|l| l.spans.is_empty()) {
        out.pop();
    }

    out
}
