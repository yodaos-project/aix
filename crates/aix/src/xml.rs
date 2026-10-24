use alloc::{format, string::String, string::ToString, vec, vec::Vec};
#[cfg(not(feature = "std"))]
use hashbrown::HashMap;
#[cfg(feature = "std")]
use std::collections::HashMap;

mod sfc;
pub use sfc::{parse_sfc_blocks, SfcBlock};

/// A lightweight XML/SFC AST node used internally by `aix`.
///
/// This type represents the parsed output of `parse_xml()` and `parse_sfc()`,
/// and is reused by AIX for page config extraction, template inspection, and
/// page size analysis.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// A regular element node.
    Element {
        /// The tag name, such as `view`, `script`, or `page`.
        name: String,
        /// The attribute map for this element.
        ///
        /// Keys are attribute names and values are attribute values. Valueless
        /// attributes are stored as empty strings.
        attributes: HashMap<String, String>,
        /// The direct child nodes of this element.
        children: Vec<Node>,
    },
    /// A text node containing raw text content.
    Text(String),
}

/// Parses XML/WXML-like text into a list of top-level nodes.
///
/// This function scans tag boundaries and builds a simplified tree structure:
///
/// - element nodes become `Node::Element`
/// - non-empty text becomes `Node::Text`
/// - loosely matched end tags are tolerated to support less strict template input
///
/// # Parameters
///
/// - `content`: The XML text to parse, typically a `.wxml` file or a template fragment.
///
/// # Returns
///
/// - `Ok(Vec<Node>)`: The parsed top-level node list.
/// - `Err(String)`: A parse error message containing the approximate position and
///   the underlying parser error.
pub fn parse_xml(content: &str) -> Result<Vec<Node>, String> {
    let mut stack: Vec<Node> = vec![Node::Element {
        name: "root".to_string(),
        attributes: HashMap::new(),
        children: Vec::new(),
    }];
    let mut cursor = 0;
    while let Some(relative_start) = content[cursor..].find('<') {
        let start = cursor + relative_start;
        let text = &content[cursor..start];
        if !text.trim().is_empty() {
            if let Some(Node::Element { children, .. }) = stack.last_mut() {
                children.push(Node::Text(text.to_string()));
            }
        }
        let Some(end) = find_tag_end(content, start + 1) else {
            return Err(format!("Unclosed tag at position {}", start));
        };
        let raw = content[start + 1..end].trim();
        cursor = end + 1;

        if raw.starts_with('!') || raw.starts_with('?') {
            continue;
        }
        if let Some(close_name) = raw.strip_prefix('/') {
            close_element(&mut stack, close_name.trim());
            continue;
        }

        let empty = raw.ends_with('/');
        let raw = raw.strip_suffix('/').unwrap_or(raw).trim_end();
        let (name, attributes) = parse_start_tag(raw)?;
        let node = Node::Element {
            name,
            attributes,
            children: Vec::new(),
        };
        if empty {
            if let Some(Node::Element { children, .. }) = stack.last_mut() {
                children.push(node);
            }
        } else {
            stack.push(node);
        }
    }
    let tail = &content[cursor..];
    if !tail.trim().is_empty() {
        if let Some(Node::Element { children, .. }) = stack.last_mut() {
            children.push(Node::Text(tail.to_string()));
        }
    }

    while stack.len() > 1 {
        let node = stack.pop().unwrap();
        if let Some(Node::Element { children, .. }) = stack.last_mut() {
            children.push(node);
        }
    }

    if let Node::Element { children, .. } = stack.pop().unwrap() {
        Ok(children)
    } else {
        Ok(Vec::new())
    }
}

fn find_tag_end(content: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, character) in content[start..].char_indices() {
        match character {
            '\'' | '"' if quote == Some(character) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(character),
            '>' if quote.is_none() => return Some(start + offset),
            _ => {}
        }
    }
    None
}

fn close_element(stack: &mut Vec<Node>, name: &str) {
    let Some(index) = (1..stack.len())
        .rev()
        .find(|&index| matches!(&stack[index], Node::Element { name: open, .. } if open == name))
    else {
        return;
    };
    while stack.len() > index + 1 {
        let child = stack.pop().unwrap();
        if let Some(Node::Element { children, .. }) = stack.last_mut() {
            children.push(child);
        }
    }
    let node = stack.pop().unwrap();
    if let Some(Node::Element { children, .. }) = stack.last_mut() {
        children.push(node);
    }
}

fn parse_start_tag(raw: &str) -> Result<(String, HashMap<String, String>), String> {
    let name_end = raw.find(char::is_whitespace).unwrap_or(raw.len());
    let name = raw[..name_end].trim();
    if name.is_empty() {
        return Err("Empty tag name".to_string());
    }
    let mut attributes = HashMap::new();
    let bytes = raw.as_bytes();
    let mut cursor = name_end;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == bytes.len() {
            break;
        }
        let key_start = cursor;
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'='
        {
            cursor += 1;
        }
        let key = &raw[key_start..cursor];
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let mut value = String::new();
        if cursor < bytes.len() && bytes[cursor] == b'=' {
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor < bytes.len() && (bytes[cursor] == b'\'' || bytes[cursor] == b'"') {
                let quote = bytes[cursor];
                cursor += 1;
                let value_start = cursor;
                while cursor < bytes.len() && bytes[cursor] != quote {
                    cursor += 1;
                }
                value = raw[value_start..cursor].to_string();
                cursor += usize::from(cursor < bytes.len());
            } else {
                let value_start = cursor;
                while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
                value = raw[value_start..cursor].to_string();
            }
        }
        attributes.insert(key.to_string(), value);
    }
    Ok((name.to_string(), attributes))
}

/// Parses `.ink` single-file component text and extracts top-level structure blocks.
///
/// This compatibility wrapper builds owned nodes from [`parse_sfc_blocks`], the
/// shared tokenizer used by metadata, layout analysis, and source diagnostics.
/// It does not implement full XML semantics and returns the following blocks:
///
/// - `script`
/// - `style`
/// - `page`
/// - `template`
///
/// For these tags, the parser preserves their attributes and stores the raw inner
/// content as a single `Node::Text` child so later stages can directly extract
/// config, template, and style text.
///
/// # Parameters
///
/// - `content`: The full contents of a `.ink` file.
///
/// # Returns
///
/// - `Ok(Vec<Node>)`: The extracted top-level SFC blocks.
/// - `Err(String)`: This return type is kept for API consistency. In normal usage,
///   this simplified parser primarily returns results through `Ok(...)`.
pub fn parse_sfc(content: &str) -> Result<Vec<Node>, String> {
    Ok(parse_sfc_blocks(content)
        .into_iter()
        .filter(|block| matches!(block.name, "script" | "style" | "page" | "template"))
        .map(|block| Node::Element {
            name: block.name.to_string(),
            attributes: block.attributes,
            children: vec![Node::Text(block.body.to_string())],
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_retains_text_and_tolerates_unmatched_and_implicit_end_tags() {
        let nodes = parse_xml("<?xml version='1.0'?><!DOCTYPE view>before<view a=one b = 'two' flag ><text>inside</view></missing>after").unwrap();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0], Node::Text("before".into()));
        assert_eq!(nodes[2], Node::Text("after".into()));
        assert_eq!(
            nodes[1],
            Node::Element {
                name: "view".into(),
                attributes: HashMap::from([
                    ("a".into(), "one".into()),
                    ("b".into(), "two".into()),
                    ("flag".into(), "".into()),
                ]),
                children: vec![Node::Element {
                    name: "text".into(),
                    attributes: HashMap::new(),
                    children: vec![Node::Text("inside".into())],
                }],
            }
        );
        assert_eq!(
            parse_xml("text only").unwrap(),
            vec![Node::Text("text only".into())]
        );
        assert_eq!(parse_xml("<view><text>tail").unwrap().len(), 1);
        assert_eq!(
            parse_xml("<view attr='unfinished").unwrap_err(),
            "Unclosed tag at position 0"
        );
        assert_eq!(parse_xml("<>").unwrap_err(), "Empty tag name");
        assert!(parse_start_tag("").is_err());
        let (_, attrs) = parse_start_tag("view attr='unterminated").unwrap();
        assert_eq!(attrs.get("attr").unwrap(), "unterminated");
    }

    #[test]
    fn sfc_ignores_comments_and_preserves_nested_markup_and_raw_text() {
        let source = r#"<!-- old example > <script def>{"title":"old"}</script> -->
<script data-note="a > b" def>{"description":"literal </script> text"}</script>
<page><page><view /></page><view class="card" /></page>
<style>.example { content: "</style>"; }.card { width: 320px; }</style>"#;
        let nodes = parse_sfc(source).unwrap();
        assert_eq!(nodes.len(), 3);
        let Node::Element {
            name,
            attributes,
            children,
        } = &nodes[0]
        else {
            panic!("expected definition block");
        };
        assert_eq!(name, "script");
        assert_eq!(
            attributes.get("data-note").map(String::as_str),
            Some("a > b")
        );
        assert!(attributes.contains_key("def"));
        assert_eq!(
            children,
            &[Node::Text(
                r#"{"description":"literal </script> text"}"#.into()
            )]
        );
        let Node::Element { children, .. } = &nodes[1] else {
            panic!("expected page block");
        };
        assert_eq!(
            children,
            &[Node::Text(
                r#"<page><view /></page><view class="card" />"#.into()
            )]
        );
        let Node::Element { children, .. } = &nodes[2] else {
            panic!("expected style block");
        };
        assert_eq!(
            children,
            &[Node::Text(
                r#".example { content: "</style>"; }.card { width: 320px; }"#.into()
            )]
        );
    }

    #[test]
    fn test_valueless_attributes() {
        let xml = "<script setup><script def></script></script>";
        let nodes = parse_xml(xml).unwrap();
        assert_eq!(nodes.len(), 1);
        if let Node::Element {
            name,
            attributes,
            children,
        } = &nodes[0]
        {
            assert_eq!(name, "script");
            assert_eq!(attributes.get("setup").unwrap(), "");
            assert_eq!(children.len(), 1);
        } else {
            panic!("Expected element");
        }
    }

    #[test]
    fn test_greater_than_inside_quoted_attribute() {
        let xml = r#"<view wx:if="{{a>b}}" id="result"></view>"#;
        let nodes = parse_xml(xml).unwrap();

        assert_eq!(nodes.len(), 1);
        if let Node::Element {
            name, attributes, ..
        } = &nodes[0]
        {
            assert_eq!(name, "view");
            assert_eq!(attributes.get("wx:if").unwrap(), "{{a>b}}");
            assert_eq!(attributes.get("id").unwrap(), "result");
        } else {
            panic!("Expected element");
        }
    }
}
