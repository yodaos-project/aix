//! Shared Ink block boundaries for metadata, layout, and source diagnostics.

use super::{parse_start_tag, HashMap};
use alloc::{string::String, vec::Vec};
use core::ops::Range;

/// One complete top-level Ink structure block, borrowing its body from the source.
///
/// Both ranges are half-open UTF-8 byte offsets in the original document. The
/// attribute map uses the same parsing rules as [`super::parse_xml`].
#[derive(Debug, Clone, PartialEq)]
pub struct SfcBlock<'a> {
    /// Block tag name: `script`, `style`, `page`, `template`, or `widget`.
    pub name: &'a str,
    /// Parsed attributes, including valueless attributes such as `def`.
    pub attributes: HashMap<String, String>,
    /// Unmodified inner text, including any nested markup.
    pub body: &'a str,
    /// Range covering the opening tag, body, and closing tag.
    pub span: Range<usize>,
    /// Range covering only the block body.
    pub body_span: Range<usize>,
}

/// Extract complete Ink blocks with source spans using the shared SFC tokenizer.
///
/// XML comments and quoted tag attributes do not introduce blocks. Markup
/// blocks balance nested tags of the same name; script/style bodies ignore
/// closing-tag text inside strings and comments. Bodies remain unmodified so
/// downstream JSON, script, and markup parsers can validate their own grammar.
///
/// This is tolerant extraction, not full syntax validation: incomplete blocks
/// are omitted, while preceding complete blocks are retained. Self-closing
/// structure tags are not treated as content blocks.
pub fn parse_sfc_blocks(source: &str) -> Vec<SfcBlock<'_>> {
    let mut found = Vec::new();
    let mut cursor = 0;
    while let Some(tag) = next_tag(source, cursor) {
        cursor = tag.end;
        if tag.close || tag.self_closing {
            continue;
        }
        let name = &source[tag.name_start..tag.name_end];
        if !matches!(name, "script" | "style" | "page" | "template" | "widget") {
            continue;
        }
        let close = if matches!(name, "script" | "style") {
            raw_close(source, tag.end, name)
        } else {
            matching_close(source, tag.end, name)
        };
        let Some(close) = close else {
            break;
        };
        cursor = close.end;
        let Ok((_, attributes)) = parse_start_tag(&source[tag.name_start..tag.end - 1]) else {
            continue;
        };
        found.push(SfcBlock {
            name,
            attributes,
            body: &source[tag.end..close.start],
            span: tag.start..close.end,
            body_span: tag.end..close.start,
        });
    }
    found
}

/// Tag/name byte ranges used for scanning; this is not an XML validation result.
struct Tag {
    start: usize,
    end: usize,
    name_start: usize,
    name_end: usize,
    close: bool,
    self_closing: bool,
}
/// Scan a tag boundary, respecting quoted `>` characters and skipping XML comments.
fn next_tag(source: &str, from: usize) -> Option<Tag> {
    let bytes = source.as_bytes();
    let mut cursor = from;
    while cursor < bytes.len() {
        let relative = source[cursor..].find('<')?;
        let start = cursor + relative;
        if source[start..].starts_with("<!--") {
            cursor = source[start + 4..]
                .find("-->")
                .map_or(bytes.len(), |end| start + 4 + end + 3);
            continue;
        }
        let mut name_start = start + 1;
        let close = bytes.get(name_start) == Some(&b'/');
        if close {
            name_start += 1;
        }
        let mut name_end = name_start;
        while bytes
            .get(name_end)
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b':'))
        {
            name_end += 1;
        }
        if name_end == name_start
            || !bytes
                .get(name_end)
                .is_some_and(|c| c.is_ascii_whitespace() || matches!(c, b'/' | b'>'))
        {
            cursor = start + 1;
            continue;
        }
        let mut end = name_end;
        let mut quote = None;
        while let Some(byte) = bytes.get(end).copied() {
            match (quote, byte) {
                (Some(current), value) if current == value => quote = None,
                (None, b'"' | b'\'') => quote = Some(byte),
                (None, b'>') => break,
                _ => {}
            }
            end += 1;
        }
        if end >= bytes.len() {
            return None;
        }
        let self_closing = bytes[name_end..end]
            .iter()
            .rev()
            .find(|c| !c.is_ascii_whitespace())
            == Some(&b'/');
        return Some(Tag {
            start,
            end: end + 1,
            name_start,
            name_end,
            close,
            self_closing,
        });
    }
    None
}

/// Balance nested markup tags, skipping script/style text rather than parsing it.
fn matching_close(source: &str, from: usize, name: &str) -> Option<Tag> {
    let mut depth = 1;
    let mut cursor = from;
    while let Some(tag) = next_tag(source, cursor) {
        cursor = tag.end;
        let tag_name = &source[tag.name_start..tag.name_end];
        if tag_name == name {
            if tag.close {
                depth -= 1;
                if depth == 0 {
                    return Some(tag);
                }
            } else if !tag.self_closing {
                depth += 1;
            }
        } else if !tag.close && matches!(tag_name, "script" | "style") {
            if let Some(close) = raw_close(source, tag.end, tag_name) {
                cursor = close.end;
            }
        }
    }
    None
}

/// Find a script/style closing tag outside quoted strings and recognized comments.
/// This scanner only establishes SFC boundaries; script/XML parsers validate the
/// extracted contents separately, and this helper is not a full JS/CSS grammar.
fn raw_close(source: &str, from: usize, name: &str) -> Option<Tag> {
    let bytes = source.as_bytes();
    let mut cursor = from;
    let mut quote = None;
    let mut line_comment = false;
    let mut block_comment = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        let next = bytes.get(cursor + 1).copied();
        if line_comment {
            if byte == b'\n' {
                line_comment = false;
            }
        } else if block_comment {
            if byte == b'*' && next == Some(b'/') {
                block_comment = false;
                cursor += 2;
                continue;
            }
        } else if let Some(current) = quote {
            if byte == b'\\' {
                cursor += 2;
                continue;
            }
            if byte == current {
                quote = None;
            }
        } else if byte == b'/' && next == Some(b'/') && name == "script" {
            line_comment = true;
            cursor += 2;
            continue;
        } else if byte == b'/' && next == Some(b'*') {
            block_comment = true;
            cursor += 2;
            continue;
        } else if matches!(byte, b'"' | b'\'' | b'`') {
            quote = Some(byte);
        } else if byte == b'<' {
            if let Some(tag) = next_tag(source, cursor) {
                if tag.start == cursor && tag.close && &source[tag.name_start..tag.name_end] == name
                {
                    return Some(tag);
                }
            }
        }
        cursor += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_utf8_spans_and_quoted_attributes() {
        let source = "😀\n<!-- old > <script>invalid</script> -->\n<script setup lang = 'ts' note=\"a > b\">\nconst message = '你好 </script>';\n</script>\n<widget><view /></widget>";
        let blocks = parse_sfc_blocks(source);
        assert_eq!(blocks.len(), 2);
        let script = &blocks[0];
        assert_eq!(script.name, "script");
        assert!(script.attributes.contains_key("setup"));
        assert_eq!(
            script.attributes.get("lang").map(String::as_str),
            Some("ts")
        );
        assert_eq!(
            script.attributes.get("note").map(String::as_str),
            Some("a > b")
        );
        assert_eq!(script.span.start, source.find("<script setup").unwrap());
        assert_eq!(&source[script.span.clone()], "<script setup lang = 'ts' note=\"a > b\">\nconst message = '你好 </script>';\n</script>");
        assert_eq!(&source[script.body_span.clone()], script.body);
        assert_eq!(script.body, "\nconst message = '你好 </script>';\n");
        assert_eq!(blocks[1].name, "widget");
        assert_eq!(&source[blocks[1].span.clone()], "<widget><view /></widget>");
    }

    #[test]
    fn balances_nested_templates_without_promoting_inner_blocks() {
        let source = r#"<template><template title="a > b"><view /></template><script def>{"fake":true}</script></template><script def>{"real":true}</script>"#;
        let blocks = parse_sfc_blocks(source);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "template");
        assert_eq!(
            blocks[0].body,
            r#"<template title="a > b"><view /></template><script def>{"fake":true}</script>"#
        );
        assert_eq!(blocks[1].name, "script");
        assert_eq!(blocks[1].body, r#"{"real":true}"#);
    }

    #[test]
    fn preserves_closing_tag_text_in_strings_escapes_and_comments() {
        let source = r#"<script setup>
const double = "\"</script>";
const single = '\'</script>';
const template = `\`</script>`;
// </script>
/* </script> */
</script>
<style>.example { content: "</style>"; } /* </style> */</style>"#;
        let blocks = parse_sfc_blocks(source);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].body.ends_with("/* </script> */\n"));
        assert_eq!(
            blocks[1].body,
            r#".example { content: "</style>"; } /* </style> */"#
        );
    }

    #[test]
    fn incomplete_blocks_keep_preceding_complete_blocks() {
        let blocks = parse_sfc_blocks("<page><view /></page><script>const incomplete = '");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "page");
        assert!(parse_sfc_blocks("<page />").is_empty());
    }
}
