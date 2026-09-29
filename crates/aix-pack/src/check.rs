//! Static checks for an AIX source tree. Diagnostics never change pack behavior.
use crate::{collector, normalize_text_to_utf8, InputFile};
use anyhow::Result;
use oxc_allocator::Allocator;
use oxc_ast::ast::{CallExpression, Expression, NewExpression};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const PERMISSIONS: &[&str] = &[
    "INTERNET",
    "GEOLOCATION",
    "CAMERA",
    "RECORD_AUDIO",
    "READ_MEDIA_IMAGES",
    "CREATE_MEDIA_IMAGES",
    "READ_MEDIA_AUDIO",
    "CREATE_MEDIA_AUDIO",
    "READ_DEVICE_SERIAL_NUMBER_LEGACY",
];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Default, Serialize)]
pub struct CheckReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
    fn add(
        &mut self,
        code: &'static str,
        severity: Severity,
        path: &str,
        source: &str,
        offset: usize,
        message: impl Into<String>,
    ) {
        let mut offset = offset.min(source.len());
        while !source.is_char_boundary(offset) {
            offset -= 1;
        }
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        self.diagnostics.push(Diagnostic {
            code,
            severity,
            path: path.into(),
            line,
            column,
            message: message.into(),
        });
    }
    fn error(
        &mut self,
        code: &'static str,
        path: &str,
        source: &str,
        offset: usize,
        message: impl Into<String>,
    ) {
        self.add(code, Severity::Error, path, source, offset, message);
    }
    fn warning(
        &mut self,
        code: &'static str,
        path: &str,
        source: &str,
        offset: usize,
        message: impl Into<String>,
    ) {
        self.add(code, Severity::Warning, path, source, offset, message);
    }
}

/// Apply the same source collection and `.aixignore` rules as `aix pack`.
pub fn check_source(files: Vec<InputFile>) -> Result<CheckReport> {
    let files = collector::collect_inputs(files, &collector::CollectOptions::default())?;
    Ok(check_files(&files))
}

pub fn check_directory(path: &Path) -> Result<CheckReport> {
    check_source(collector::read_directory(path)?)
}

/// Check already collected package inputs without writing an artifact.
pub fn check_files(files: &[InputFile]) -> CheckReport {
    let entries: BTreeMap<&str, &[u8]> = files
        .iter()
        .map(|f| (f.path.as_str(), f.data.as_slice()))
        .collect();
    let mut report = CheckReport::default();
    let mut texts = BTreeMap::<&str, String>::new();
    let mut jsons = BTreeMap::<&str, Value>::new();
    for file in files {
        if !file.path.ends_with(".json") && !is_source(&file.path) {
            continue;
        }
        match normalize_text_to_utf8(&file.path, &file.data)
            .and_then(|(bytes, _)| String::from_utf8(bytes).map_err(Into::into))
        {
            Ok(content) => {
                if file.path.ends_with(".json") {
                    match serde_json::from_str(&content) {
                        Ok(value) => {
                            jsons.insert(&file.path, value);
                        }
                        Err(error) => report.error(
                            "AIX001",
                            &file.path,
                            &content,
                            json_error_offset(&content, error.line(), error.column()),
                            format!("Invalid JSON: {error}"),
                        ),
                    }
                }
                texts.insert(&file.path, content);
            }
            Err(error) => report.error("AIX002", &file.path, "", 0, error.to_string()),
        }
    }
    let Some(app) = jsons.get("app.json") else {
        if !entries.contains_key("app.json") {
            report.error("AIX003", "app.json", "", 0, "app.json is required");
        }
        return finish(report);
    };
    let app_text = texts.get("app.json").map(String::as_str).unwrap_or("");
    let Some(app) = app.as_object() else {
        report.error(
            "AIX004",
            "app.json",
            app_text,
            0,
            "app.json must be an object",
        );
        return finish(report);
    };
    if let Some(engine) = app.get("engine") {
        match engine.as_str() {
            Some(range) => {
                if let Err(error) = aix::crypto::validate_engine_range(range) {
                    report.error(
                        "AIX005",
                        "app.json",
                        app_text,
                        field_offset(app_text, "engine"),
                        format!("Invalid engine range: {error}"),
                    );
                }
            }
            None => report.error(
                "AIX005",
                "app.json",
                app_text,
                field_offset(app_text, "engine"),
                "engine must be a string",
            ),
        }
    }
    if let Some(window) = app.get("window") {
        if !window.is_object() {
            report.error(
                "AIX034",
                "app.json",
                app_text,
                field_offset(app_text, "window"),
                "window must be an object",
            );
        }
    }
    let mut permissions = BTreeSet::new();
    if let Some(value) = app.get("permissions") {
        if let Some(list) = value.as_array() {
            let positions = object_field(app_text, 0, "permissions")
                .map(|(_, start, _)| array_items(app_text, start))
                .unwrap_or_default();
            for (index, item) in list.iter().enumerate() {
                let offset = positions
                    .get(index)
                    .map_or_else(|| field_offset(app_text, "permissions"), |range| range.0);
                match item.as_str() {
                    Some(permission) => {
                        if !PERMISSIONS.contains(&permission) {
                            report.warning(
                                "AIX006",
                                "app.json",
                                app_text,
                                offset,
                                format!("Unknown Ink permission {permission}"),
                            );
                        }
                        if !permissions.insert(permission.to_string()) {
                            report.warning(
                                "AIX007",
                                "app.json",
                                app_text,
                                offset,
                                format!("Duplicate permission {permission}"),
                            );
                        }
                    }
                    None => report.error(
                        "AIX037",
                        "app.json",
                        app_text,
                        offset,
                        "permissions must contain only strings",
                    ),
                }
            }
        } else {
            report.error(
                "AIX038",
                "app.json",
                app_text,
                field_offset(app_text, "permissions"),
                "permissions must be an array of strings",
            );
        }
    }
    check_components(
        app.get("usingComponents"),
        "app.json",
        app_text,
        &entries,
        &jsons,
        &texts,
        &mut report,
    );
    let mut declared = BTreeSet::new();
    match app.get("pages").and_then(Value::as_array) {
        None => report.error(
            "AIX008",
            "app.json",
            app_text,
            field_offset(app_text, "pages"),
            "pages must be an array of paths",
        ),
        Some(pages) => {
            let positions = object_field(app_text, 0, "pages")
                .map(|(_, start, _)| array_items(app_text, start))
                .unwrap_or_default();
            for (index, page) in pages.iter().enumerate() {
                let offset = positions
                    .get(index)
                    .map_or_else(|| field_offset(app_text, "pages"), |range| range.0);
                match page.as_str() {
                    Some(path) if valid_path(path) => {
                        if !declared.insert(path) {
                            report.error(
                                "AIX009",
                                "app.json",
                                app_text,
                                offset,
                                format!("Duplicate entry path {path}"),
                            );
                            continue;
                        }
                        check_page(path, &entries, &jsons, &texts, &mut report);
                    }
                    Some(path) => report.error(
                        "AIX039",
                        "app.json",
                        app_text,
                        offset,
                        format!("Invalid page path {path:?}; expected a non-empty extensionless relative path"),
                    ),
                    None => report.error(
                        "AIX040",
                        "app.json",
                        app_text,
                        offset,
                        "Page entry must be a string path",
                    ),
                }
            }
        }
    }
    if let Some(widgets) = app.get("widgets") {
        match widgets.as_array() {
            None => report.error(
                "AIX010",
                "app.json",
                app_text,
                field_offset(app_text, "widgets"),
                "widgets must be an array",
            ),
            Some(widgets) => {
                let positions = object_field(app_text, 0, "widgets")
                    .map(|(_, start, _)| array_items(app_text, start))
                    .unwrap_or_default();
                for (index, widget) in widgets.iter().enumerate() {
                    let widget_start = positions
                        .get(index)
                        .map_or_else(|| field_offset(app_text, "widgets"), |range| range.0);
                    let widget_field = |name| {
                        object_field(app_text, widget_start, name)
                            .map_or(widget_start, |(key, _, _)| key)
                    };
                    let Some(obj) = widget.as_object() else {
                        report.error(
                            "AIX043",
                            "app.json",
                            app_text,
                            widget_start,
                            "Widget must be an object",
                        );
                        continue;
                    };
                    let path = obj.get("path").and_then(Value::as_str).unwrap_or("");
                    let family = obj.get("family").and_then(Value::as_str).unwrap_or("");
                    if !valid_path(path) {
                        report.error(
                            "AIX044",
                            "app.json",
                            app_text,
                            widget_field("path"),
                            "Widget path must be a non-empty extensionless relative path",
                        );
                        continue;
                    }
                    if !declared.insert(path) {
                        report.error(
                            "AIX009",
                            "app.json",
                            app_text,
                            widget_field("path"),
                            format!("Duplicate entry path {path}"),
                        );
                        continue;
                    }
                    if !matches!(family, "1x1" | "1x2") {
                        report.error(
                            "AIX011",
                            "app.json",
                            app_text,
                            widget_field("family"),
                            format!("Unsupported Widget family {family:?}; expected 1x1 or 1x2"),
                        );
                    }
                    if let Some(placement) = obj.get("placement") {
                        if !matches!(placement.as_str(), Some("persistent" | "overlay")) {
                            report.error(
                                "AIX012",
                                "app.json",
                                app_text,
                                widget_field("placement"),
                                "Widget placement must be persistent or overlay",
                            );
                        }
                    }
                    for field in ["displayName", "description"] {
                        if obj
                            .get(field)
                            .is_some_and(|v| !v.is_null() && !v.is_string())
                        {
                            report.error(
                                "AIX035",
                                "app.json",
                                app_text,
                                widget_field(field),
                                format!("Widget {field} must be a string or null"),
                            );
                        }
                    }
                    check_widget(path, family, &entries, &jsons, &texts, &mut report);
                }
            }
        }
    }
    for (path, content) in &texts {
        if path.ends_with(".ink") && !declared.contains(path.trim_end_matches(".ink")) {
            if let Some(def) = definition_if_present(path, &texts, &mut report) {
                if def.get("component").and_then(Value::as_bool) == Some(true) {
                    let template_blocks = blocks(content, "template");
                    if template_blocks.len() != 1 {
                        report.error(
                            "AIX025",
                            path,
                            content,
                            0,
                            "Component must contain exactly one <template> block",
                        );
                    }
                    for block in &template_blocks {
                        check_markup(path, content, block, &mut report);
                    }
                    check_components(
                        def.get("usingComponents"),
                        path,
                        content,
                        &entries,
                        &jsons,
                        &texts,
                        &mut report,
                    );
                }
            }
        }
        if path.ends_with(".js")
            || path.ends_with(".ts")
            || path.ends_with(".mjs")
            || path.ends_with(".mts")
        {
            check_script(
                path,
                content,
                0,
                content,
                path.ends_with(".ts") || path.ends_with(".mts"),
                &permissions,
                &mut report,
            );
        } else if path.ends_with(".ink") {
            for block in blocks(content, "script") {
                if !block
                    .attributes
                    .split_whitespace()
                    .any(|part| part == "def")
                {
                    check_script(
                        path,
                        block.body,
                        block.offset,
                        content,
                        block.attributes.contains("lang=\"ts\"")
                            || block.attributes.contains("lang='ts'"),
                        &permissions,
                        &mut report,
                    );
                }
            }
        }
    }
    for (path, value) in &jsons {
        if path.ends_with(".json") && value.get("component").and_then(Value::as_bool) == Some(true)
        {
            check_components(
                value.get("usingComponents"),
                path,
                texts.get(path).map(String::as_str).unwrap_or(""),
                &entries,
                &jsons,
                &texts,
                &mut report,
            );
        }
    }
    check_component_cycles(&texts, &jsons, &mut report);
    finish(report)
}

fn finish(mut report: CheckReport) -> CheckReport {
    report.diagnostics.sort_by(|a, b| {
        (&a.path, a.line, a.column, a.code).cmp(&(&b.path, b.line, b.column, b.code))
    });
    report.diagnostics.dedup_by(|a, b| {
        a.path == b.path
            && a.line == b.line
            && a.column == b.column
            && a.code == b.code
            && a.message == b.message
    });
    report
}
fn is_source(path: &str) -> bool {
    [".ink", ".js", ".ts", ".mjs", ".mts"]
        .iter()
        .any(|ext| path.ends_with(ext))
}
fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.starts_with(aix::crypto::METADATA_PREFIX)
        && !path.chars().any(|c| c.is_control() || c == ' ')
        && ![".ink", ".json", ".wxml", ".wxss", ".wcss"]
            .iter()
            .any(|ext| path.ends_with(ext))
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
fn field_offset(source: &str, field: &str) -> usize {
    object_field(source, 0, field)
        .map(|(key, _, _)| key)
        .or_else(|| source.find(&format!("\"{field}\"")))
        .unwrap_or(0)
}
fn component_value_offset(source: &str, tag: &str) -> usize {
    source
        .find("\"usingComponents\"")
        .and_then(|key| source[key..].find('{').map(|relative| key + relative))
        .and_then(|object| object_field(source, object, tag).map(|(_, value, _)| value))
        .unwrap_or_else(|| field_offset(source, tag))
}
fn json_error_offset(source: &str, line: usize, column: usize) -> usize {
    // serde_json counts UTF-8 bytes, not Unicode scalar values, in columns.
    // Clamp later in CheckReport::add because an error can point into an
    // incomplete multibyte sequence.
    source
        .split_inclusive('\n')
        .take(line.saturating_sub(1))
        .map(str::len)
        .sum::<usize>()
        + column.saturating_sub(1)
}

fn skip_space(source: &str, mut offset: usize) -> usize {
    let bytes = source.as_bytes();
    while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
        offset += 1;
    }
    offset
}

fn string_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut offset = start + 1;
    while offset < bytes.len() {
        match bytes[offset] {
            b'\\' => offset += 2,
            b'"' => return Some(offset + 1),
            _ => offset += 1,
        }
    }
    None
}

// Input is already validated JSON. These helpers retain source spans while
// serde_json::Value supplies the semantic value of each member.
fn value_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut offset = skip_space(source, start);
    match *bytes.get(offset)? {
        b'"' => string_end(source, offset),
        b'{' => {
            offset += 1;
            loop {
                offset = skip_space(source, offset);
                if bytes.get(offset) == Some(&b'}') {
                    return Some(offset + 1);
                }
                offset = string_end(source, offset)?;
                offset = skip_space(source, offset);
                if bytes.get(offset) != Some(&b':') {
                    return None;
                }
                offset = value_end(source, offset + 1)?;
                offset = skip_space(source, offset);
                match bytes.get(offset)? {
                    b',' => offset += 1,
                    b'}' => return Some(offset + 1),
                    _ => return None,
                }
            }
        }
        b'[' => {
            offset += 1;
            loop {
                offset = skip_space(source, offset);
                if bytes.get(offset) == Some(&b']') {
                    return Some(offset + 1);
                }
                offset = value_end(source, offset)?;
                offset = skip_space(source, offset);
                match bytes.get(offset)? {
                    b',' => offset += 1,
                    b']' => return Some(offset + 1),
                    _ => return None,
                }
            }
        }
        _ => {
            while offset < bytes.len()
                && !matches!(
                    bytes[offset],
                    b',' | b'}' | b']' | b' ' | b'\n' | b'\r' | b'\t'
                )
            {
                offset += 1;
            }
            Some(offset)
        }
    }
}

fn object_field(source: &str, start: usize, field: &str) -> Option<(usize, usize, usize)> {
    let bytes = source.as_bytes();
    let mut offset = skip_space(source, start);
    if bytes.get(offset)? != &b'{' {
        return None;
    }
    offset += 1;
    loop {
        offset = skip_space(source, offset);
        if bytes.get(offset) == Some(&b'}') {
            return None;
        }
        let key_start = offset;
        let key_end = string_end(source, offset)?;
        let key: String = serde_json::from_str(source.get(key_start..key_end)?).ok()?;
        offset = skip_space(source, key_end);
        if bytes.get(offset) != Some(&b':') {
            return None;
        }
        let value_start = skip_space(source, offset + 1);
        let end = value_end(source, value_start)?;
        if key == field {
            return Some((key_start, value_start, end));
        }
        offset = skip_space(source, end);
        match bytes.get(offset)? {
            b',' => offset += 1,
            b'}' => return None,
            _ => return None,
        }
    }
}

fn array_items(source: &str, start: usize) -> Vec<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut offset = skip_space(source, start);
    if bytes.get(offset) != Some(&b'[') {
        return Vec::new();
    }
    offset += 1;
    let mut items = Vec::new();
    loop {
        offset = skip_space(source, offset);
        if bytes.get(offset) == Some(&b']') {
            break;
        }
        let Some(end) = value_end(source, offset) else {
            break;
        };
        items.push((offset, end));
        offset = skip_space(source, end);
        match bytes.get(offset) {
            Some(b',') => offset += 1,
            _ => break,
        }
    }
    items
}

struct Block<'a> {
    attributes: &'a str,
    body: &'a str,
    offset: usize,
}
fn blocks<'a>(source: &'a str, name: &str) -> Vec<Block<'a>> {
    let mut found = Vec::new();
    let mut cursor = 0;
    while let Some(tag) = next_tag(source, cursor) {
        cursor = tag.end;
        if tag.close || tag.self_closing {
            continue;
        }
        let tag_name = &source[tag.name_start..tag.name_end];
        if tag_name != name {
            if matches!(tag_name, "script" | "style") {
                if let Some(close) = raw_close(source, tag.end, tag_name) {
                    cursor = close.end;
                }
            }
            continue;
        }
        let Some(close) = (if matches!(name, "script" | "style") {
            raw_close(source, tag.end, name)
        } else {
            matching_close(source, tag.end, name)
        }) else {
            break;
        };
        found.push(Block {
            attributes: &source[tag.name_end..tag.end - 1],
            body: &source[tag.end..close.start],
            offset: tag.end,
        });
        cursor = close.end;
    }
    found
}

struct Tag {
    start: usize,
    end: usize,
    name_start: usize,
    name_end: usize,
    close: bool,
    self_closing: bool,
}
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
fn check_markup(path: &str, full: &str, block: &Block<'_>, report: &mut CheckReport) {
    if let Err(error) = aix::xml::parse_xml(block.body) {
        report.error(
            "AIX036",
            path,
            full,
            block.offset,
            format!("Invalid markup: {error}"),
        );
    }
}
fn definition(
    path: &str,
    texts: &BTreeMap<&str, String>,
    report: &mut CheckReport,
) -> Option<Value> {
    let content = texts.get(path)?;
    let defs: Vec<_> = blocks(content, "script")
        .into_iter()
        .filter(|b| b.attributes.split_whitespace().any(|p| p == "def"))
        .collect();
    if defs.len() != 1 {
        report.error(
            "AIX013",
            path,
            content,
            0,
            "Expected exactly one <script def> block",
        );
        return None;
    }
    match serde_json::from_str(defs[0].body) {
        Ok(value) => Some(value),
        Err(error) => {
            report.error(
                "AIX014",
                path,
                content,
                defs[0].offset + json_error_offset(defs[0].body, error.line(), error.column()),
                format!("Invalid <script def> JSON: {error}"),
            );
            None
        }
    }
}
fn definition_if_present(
    path: &str,
    texts: &BTreeMap<&str, String>,
    report: &mut CheckReport,
) -> Option<Value> {
    let content = texts.get(path)?;
    if blocks(content, "script")
        .iter()
        .any(|b| b.attributes.split_whitespace().any(|p| p == "def"))
    {
        definition(path, texts, report)
    } else {
        None
    }
}
fn check_page(
    path: &str,
    entries: &BTreeMap<&str, &[u8]>,
    jsons: &BTreeMap<&str, Value>,
    texts: &BTreeMap<&str, String>,
    report: &mut CheckReport,
) {
    let ink = format!("{path}.ink");
    if entries.contains_key(ink.as_str()) {
        let Some(content) = texts.get(ink.as_str()) else {
            return;
        };
        let page_blocks = blocks(content, "page");
        if page_blocks.len() != 1 {
            report.error(
                "AIX015",
                &ink,
                content,
                0,
                "Page must contain exactly one <page> block",
            );
        }
        for block in &page_blocks {
            check_markup(&ink, content, block, report);
        }
        if !blocks(content, "widget").is_empty() {
            report.error(
                "AIX041",
                &ink,
                content,
                0,
                "Page cannot contain a <widget> block",
            );
        }
        if let Some(def) = definition_if_present(&ink, texts, report) {
            if !def.is_object() {
                report.error(
                    "AIX030",
                    &ink,
                    content,
                    0,
                    "Page <script def> must be an object",
                );
            }
            check_page_schema(&def, &ink, content, report);
            check_components(
                def.get("usingComponents"),
                &ink,
                content,
                entries,
                jsons,
                texts,
                report,
            );
        }
    } else {
        let config = format!("{path}.json");
        let markup = format!("{path}.wxml");
        if !entries.contains_key(config.as_str()) {
            report.error("AIX016", &config, "", 0, "Page config file is missing");
        }
        if !entries.contains_key(markup.as_str()) {
            report.error("AIX016", &markup, "", 0, "Page markup file is missing");
        }
        if let Some(content) = texts.get(markup.as_str()) {
            let block = Block {
                attributes: "",
                body: content,
                offset: 0,
            };
            check_markup(&markup, content, &block, report);
        }
        if let Some(def) = jsons.get(config.as_str()) {
            let content = texts.get(config.as_str()).map(String::as_str).unwrap_or("");
            if !def.is_object() {
                report.error(
                    "AIX030",
                    &config,
                    content,
                    0,
                    "Page config must be an object",
                );
            }
            check_page_schema(def, &config, content, report);
            check_components(
                def.get("usingComponents"),
                &config,
                content,
                entries,
                jsons,
                texts,
                report,
            );
        }
    }
}
fn check_page_schema(def: &Value, path: &str, content: &str, report: &mut CheckReport) {
    if let Some(schema) = def.get("schema") {
        if !schema.is_null() && !schema.is_object() {
            report.error(
                "AIX031",
                path,
                content,
                field_offset(content, "schema"),
                "Page schema must be an object or null",
            );
        }
    }
}
fn check_component_cycles(
    texts: &BTreeMap<&str, String>,
    jsons: &BTreeMap<&str, Value>,
    report: &mut CheckReport,
) {
    let mut edges = BTreeMap::<String, Vec<String>>::new();
    for (path, content) in texts {
        if !path.ends_with(".ink") {
            continue;
        }
        let Some(def) = blocks(content, "script")
            .into_iter()
            .find(|b| b.attributes.split_whitespace().any(|p| p == "def"))
            .and_then(|b| serde_json::from_str::<Value>(b.body).ok())
        else {
            continue;
        };
        if def.get("component").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let dependencies = def
            .get("usingComponents")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|obj| obj.values())
            .filter_map(Value::as_str)
            .filter(|p| !p.starts_with('@'))
            .map(|p| {
                let normalized = p.trim_start_matches('/');
                let ink = format!("{normalized}.ink");
                if texts.contains_key(ink.as_str()) {
                    ink
                } else {
                    format!("{normalized}.json")
                }
            })
            .collect();
        edges.insert((*path).to_string(), dependencies);
    }
    for (path, def) in jsons {
        if def.get("component").and_then(Value::as_bool) != Some(true) || !path.ends_with(".json") {
            continue;
        }
        let dependencies = def
            .get("usingComponents")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|obj| obj.values())
            .filter_map(Value::as_str)
            .filter(|p| !p.starts_with('@'))
            .map(|p| {
                let normalized = p.trim_start_matches('/');
                let ink = format!("{normalized}.ink");
                if texts.contains_key(ink.as_str()) {
                    ink
                } else {
                    format!("{normalized}.json")
                }
            })
            .collect();
        edges.insert((*path).to_string(), dependencies);
    }
    fn visit(
        node: &str,
        edges: &BTreeMap<String, Vec<String>>,
        active: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
        texts: &BTreeMap<&str, String>,
        report: &mut CheckReport,
    ) {
        if done.contains(node) {
            return;
        }
        if !active.insert(node.to_string()) {
            return;
        }
        if let Some(next) = edges.get(node) {
            for dependency in next {
                if active.contains(dependency) {
                    report.error(
                        "AIX032",
                        node,
                        texts.get(node).map(String::as_str).unwrap_or(""),
                        0,
                        format!("Component reference cycle through {dependency}"),
                    );
                } else if edges.contains_key(dependency) {
                    visit(dependency, edges, active, done, texts, report);
                }
            }
        }
        active.remove(node);
        done.insert(node.to_string());
    }
    let mut done = BTreeSet::new();
    for node in edges.keys() {
        visit(node, &edges, &mut BTreeSet::new(), &mut done, texts, report);
    }
}
fn check_widget(
    path: &str,
    family: &str,
    entries: &BTreeMap<&str, &[u8]>,
    jsons: &BTreeMap<&str, Value>,
    texts: &BTreeMap<&str, String>,
    report: &mut CheckReport,
) {
    let ink = format!("{path}.ink");
    if !entries.contains_key(ink.as_str()) {
        report.error("AIX017", &ink, "", 0, "Widget .ink entry is missing");
        return;
    }
    let Some(content) = texts.get(ink.as_str()) else {
        return;
    };
    let widget_blocks = blocks(content, "widget");
    if widget_blocks.len() != 1 {
        report.error(
            "AIX018",
            &ink,
            content,
            0,
            "Widget must contain exactly one <widget> block",
        );
    }
    if !blocks(content, "page").is_empty() {
        report.error(
            "AIX046",
            &ink,
            content,
            0,
            "Widget cannot contain a <page> block",
        );
    }
    for block in &widget_blocks {
        check_markup(&ink, content, block, report);
    }
    if let Some(def) = definition(&ink, texts, report) {
        if def.pointer("/widget/family").and_then(Value::as_str) != Some(family) {
            report.error(
                "AIX019",
                &ink,
                content,
                field_offset(content, "family"),
                format!("Widget family must match app.json ({family})"),
            );
        }
        check_components(
            def.get("usingComponents"),
            &ink,
            content,
            entries,
            jsons,
            texts,
            report,
        );
    }
}
fn check_components(
    value: Option<&Value>,
    owner: &str,
    content: &str,
    entries: &BTreeMap<&str, &[u8]>,
    jsons: &BTreeMap<&str, Value>,
    texts: &BTreeMap<&str, String>,
    report: &mut CheckReport,
) {
    let Some(value) = value else {
        return;
    };
    let Some(components) = value.as_object() else {
        report.error(
            "AIX020",
            owner,
            content,
            field_offset(content, "usingComponents"),
            "usingComponents must be an object",
        );
        return;
    };
    for (tag, value) in components {
        let reference_offset = component_value_offset(content, tag);
        let Some(path) = value.as_str() else {
            report.error(
                "AIX045",
                owner,
                content,
                reference_offset,
                format!("Component {tag} path must be a string"),
            );
            continue;
        };
        if !tag.contains('-') {
            report.warning(
                "AIX021",
                owner,
                content,
                reference_offset,
                format!("Custom component tag {tag} should contain a hyphen"),
            );
        }
        if path.starts_with('@') {
            report.warning(
                "AIX022",
                owner,
                content,
                reference_offset,
                format!("Host component {path} cannot be verified locally"),
            );
            continue;
        }
        let normalized = path.trim_start_matches('/');
        if !valid_path(normalized) {
            report.error(
                "AIX023",
                owner,
                content,
                reference_offset,
                format!("Invalid component path {path}"),
            );
            continue;
        }
        let entry = format!("{normalized}.ink");
        let legacy = format!("{normalized}.json");
        if !entries.contains_key(entry.as_str()) && !entries.contains_key(legacy.as_str()) {
            report.error(
                "AIX024",
                owner,
                content,
                reference_offset,
                format!("Component entry {entry} or {legacy} is missing"),
            );
            continue;
        }
        if let Some(component_content) = texts.get(entry.as_str()) {
            let template_blocks = blocks(component_content, "template");
            if template_blocks.len() != 1 {
                report.error(
                    "AIX025",
                    &entry,
                    component_content,
                    0,
                    "Component must contain exactly one <template> block",
                );
            }
            for block in &template_blocks {
                check_markup(&entry, component_content, block, report);
            }
            if let Some(def) = definition_if_present(&entry, texts, report) {
                if def
                    .get("component")
                    .is_some_and(|value| value.as_bool() != Some(true))
                {
                    report.error(
                        "AIX026",
                        &entry,
                        component_content,
                        field_offset(component_content, "component"),
                        "Component <script def> component field must be true",
                    );
                }
            }
        } else if entries.contains_key(legacy.as_str()) {
            let markup = format!("{normalized}.wxml");
            if !entries.contains_key(markup.as_str()) {
                report.error(
                    "AIX042",
                    owner,
                    content,
                    reference_offset,
                    format!("Component markup file {markup} is missing"),
                );
            }
            if let Some(markup_content) = texts.get(markup.as_str()) {
                let block = Block {
                    attributes: "",
                    body: markup_content,
                    offset: 0,
                };
                check_markup(&markup, markup_content, &block, report);
            }
            if let Some(def) = jsons.get(legacy.as_str()) {
                let config = texts.get(legacy.as_str()).map(String::as_str).unwrap_or("");
                if def
                    .get("component")
                    .is_some_and(|value| value.as_bool() != Some(true))
                {
                    report.error(
                        "AIX026",
                        &legacy,
                        config,
                        field_offset(config, "component"),
                        "Component config component field must be true",
                    );
                }
            }
        }
    }
}

struct ScriptVisitor<'a, 'b> {
    path: &'b str,
    full: &'b str,
    base: usize,
    permissions: &'b BTreeSet<String>,
    report: &'b mut CheckReport,
    _marker: std::marker::PhantomData<&'a ()>,
}
impl<'a> Visit<'a> for ScriptVisitor<'a, '_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let name = callee_name(&call.callee);
        if let Some(name) = name {
            match name.as_str() {
                "navigator.geolocation.getCurrentPosition"
                | "navigator.geolocation.watchPosition" => {
                    self.require("GEOLOCATION", call.span.start as usize)
                }
                "navigator.mediaDevices.getUserMedia" => self.media_constraints(call),
                "navigator.mediaLibrary.list" | "navigator.mediaLibrary.get" => {
                    self.media_read(call)
                }
                "navigator.mediaLibrary.save" => self.unknown(
                    call.span.start as usize,
                    "mediaLibrary.save() Blob MIME type",
                ),
                "fetch" | "wx.request" | "wx.downloadFile" | "wx.uploadFile" => {
                    self.network(call.span.start as usize)
                }
                _ => {}
            }
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_new_expression(&mut self, expression: &NewExpression<'a>) {
        if matches!(
            callee_name(&expression.callee).as_deref(),
            Some("WebSocket" | "EventSource" | "XMLHttpRequest")
        ) {
            self.network(expression.span.start as usize);
        }
        walk::walk_new_expression(self, expression);
    }
}
impl ScriptVisitor<'_, '_> {
    fn require(&mut self, permission: &str, offset: usize) {
        if !self.permissions.contains(permission) {
            self.report.error(
                "AIX027",
                self.path,
                self.full,
                self.base + offset,
                format!("API use requires app.json.permissions to include {permission}"),
            );
        }
    }
    fn media_constraints(&mut self, call: &CallExpression<'_>) {
        let Some(first) = call.arguments.first() else {
            self.unknown(call.span.start as usize, "getUserMedia constraints");
            return;
        };
        let Some(expression) = first.as_expression() else {
            self.unknown(call.span.start as usize, "getUserMedia constraints");
            return;
        };
        let Expression::ObjectExpression(object) = expression else {
            self.unknown(call.span.start as usize, "getUserMedia constraints");
            return;
        };
        let mut known = false;
        for property in &object.properties {
            let Some(property) = property.as_property() else {
                continue;
            };
            let Some(key) = property.key.static_name() else {
                continue;
            };
            if key == "audio" || key == "video" {
                known = true;
                match &property.value {
                    Expression::BooleanLiteral(value) if !value.value => {}
                    Expression::BooleanLiteral(value) if value.value => self.require(
                        if key == "audio" {
                            "RECORD_AUDIO"
                        } else {
                            "CAMERA"
                        },
                        call.span.start as usize,
                    ),
                    Expression::ObjectExpression(_) => self.require(
                        if key == "audio" {
                            "RECORD_AUDIO"
                        } else {
                            "CAMERA"
                        },
                        call.span.start as usize,
                    ),
                    _ => self.unknown(call.span.start as usize, "getUserMedia constraints"),
                }
            }
        }
        if !known {
            self.unknown(call.span.start as usize, "getUserMedia constraints");
        }
    }
    fn media_read(&mut self, call: &CallExpression<'_>) {
        let index = if callee_name(&call.callee).as_deref() == Some("navigator.mediaLibrary.get") {
            1
        } else {
            0
        };
        let media = match call.arguments.get(index) {
            None => Some("image"),
            Some(argument) => match argument.as_expression() {
                Some(Expression::ObjectExpression(object)) => {
                    match object
                        .properties
                        .iter()
                        .filter_map(|p| p.as_property())
                        .find(|p| p.key.static_name().as_deref() == Some("mediaType"))
                    {
                        None => Some("image"),
                        Some(property) => match &property.value {
                            Expression::StringLiteral(value) => Some(value.value.as_str()),
                            _ => None,
                        },
                    }
                }
                _ => None,
            },
        };
        match media {
            Some("audio") => self.require("READ_MEDIA_AUDIO", call.span.start as usize),
            Some("image") => self.require("READ_MEDIA_IMAGES", call.span.start as usize),
            _ => self.unknown(call.span.start as usize, "mediaLibrary mediaType"),
        }
    }
    fn unknown(&mut self, offset: usize, what: &str) {
        self.report.warning(
            "AIX028",
            self.path,
            self.full,
            self.base + offset,
            format!("Cannot determine {what} statically; verify app.json.permissions"),
        );
    }
    fn network(&mut self, offset: usize) {
        if !self.permissions.contains("INTERNET") {
            self.report.warning("AIX033", self.path, self.full, self.base + offset, "Network API is used without INTERNET in app.json.permissions; Ink does not currently gate every network API");
        }
    }
}
fn callee_name(expression: &Expression<'_>) -> Option<String> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        other => {
            let member = other.get_member_expr()?;
            Some(format!(
                "{}.{}",
                callee_name(member.object())?,
                member.static_property_name()?
            ))
        }
    }
}
fn check_script(
    path: &str,
    source: &str,
    base: usize,
    full: &str,
    typescript: bool,
    permissions: &BTreeSet<String>,
    report: &mut CheckReport,
) {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(Path::new(if typescript {
        "inline.ts"
    } else {
        "inline.js"
    }))
    .unwrap_or_default()
    .with_module(true);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        for error in parsed.diagnostics.iter() {
            let offset = error
                .labels
                .as_slice()
                .first()
                .map_or(0, |label| label.offset() as usize);
            report.error(
                "AIX029",
                path,
                full,
                base + offset,
                format!("Invalid JS/TS: {error}"),
            );
        }
        if parsed.panicked && parsed.diagnostics.is_empty() {
            report.error("AIX029", path, full, base, "JS/TS parser failed");
        }
        return;
    }
    let mut visitor = ScriptVisitor {
        path,
        full,
        base,
        permissions,
        report,
        _marker: std::marker::PhantomData,
    };
    visitor.visit_program(&parsed.program);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(app: &str, more: &[(&str, &str)]) -> CheckReport {
        let mut files = vec![InputFile::new("app.json", app.as_bytes())];
        files.extend(more.iter().map(|(p, s)| InputFile::new(*p, s.as_bytes())));
        check_source(files).unwrap()
    }
    #[test]
    fn detects_missing_entry_and_permission() {
        let report = check(
            r#"{"pages":["pages/main/index"],"permissions":[]}"#,
            &[("app.js", "navigator.geolocation.watchPosition(() => {});")],
        );
        assert!(report.diagnostics.iter().any(|d| d.code == "AIX016"));
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.code == "AIX027" && d.message.contains("GEOLOCATION")));
    }
    #[test]
    fn checks_widget_family_and_component_entry() {
        let report = check(r#"{"pages":[],"widgets":[{"path":"widgets/clock/index","family":"1x2"}]}"#, &[("widgets/clock/index.ink", "<script def>{\"widget\":{\"family\":\"1x1\"},\"usingComponents\":{\"my-card\":\"components/card\"}}</script><widget><view /></widget>")]);
        assert!(report.diagnostics.iter().any(|d| d.code == "AIX019"));
        assert!(report.diagnostics.iter().any(|d| d.code == "AIX024"));
    }
    #[test]
    fn respects_declared_media_permissions() {
        let report = check(
            r#"{"pages":[],"permissions":["CAMERA"]}"#,
            &[(
                "app.ts",
                "navigator.mediaDevices.getUserMedia({video:true,audio:false});",
            )],
        );
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
    }
    #[test]
    fn reports_dynamic_media_constraints_without_guessing() {
        let report = check(
            r#"{"pages":[]}"#,
            &[(
                "app.ts",
                "navigator.mediaDevices.getUserMedia(constraints);",
            )],
        );
        assert!(report.diagnostics.iter().any(|d| d.code == "AIX028"));
        assert!(!report.has_errors());
    }
    #[test]
    fn detects_component_cycle() {
        let report = check(r#"{"pages":[],"usingComponents":{"first-card":"components/first"}}"#, &[
            ("components/first.ink", "<script def>{\"component\":true,\"usingComponents\":{\"second-card\":\"components/second\"}}</script><template><view /></template>"),
            ("components/second.ink", "<script def>{\"component\":true,\"usingComponents\":{\"first-card\":\"components/first\"}}</script><template><view /></template>"),
        ]);
        assert!(report.diagnostics.iter().any(|d| d.code == "AIX032"));
    }
    #[test]
    fn honors_aixignore() {
        let report = check_source(vec![
            InputFile::new("app.json", br#"{"pages":[]}"#),
            InputFile::new(".aixignore", b"ignored.js\n"),
            InputFile::new(
                "ignored.js",
                b"navigator.geolocation.watchPosition(() => {});",
            ),
        ])
        .unwrap();
        assert!(!report.has_errors());
    }
    #[test]
    fn accepts_legacy_component() {
        let report = check(
            r#"{"pages":[],"usingComponents":{"old-card":"components/old"}}"#,
            &[
                ("components/old.json", r#"{"component":true}"#),
                ("components/old.wxml", "<view />"),
            ],
        );
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
    }
    #[test]
    fn checks_inline_typescript_and_ignores_comment_text() {
        let report = check(
            r#"{"pages":["pages/home/index"],"permissions":["GEOLOCATION"]}"#,
            &[(
                "pages/home/index.ink",
                "<script setup lang=\"ts\">const count: number = 1; // navigator.mediaDevices.getUserMedia({audio:true})\nnavigator.geolocation.getCurrentPosition(() => count);</script><page><view /></page>",
            )],
        );
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
        assert!(!report.diagnostics.iter().any(|d| d.code == "AIX027"));
    }
    #[test]
    fn accepts_dotted_logical_path_segment() {
        let report = check(
            r#"{"pages":["pages/v1.2/home"]}"#,
            &[
                ("pages/v1.2/home.json", "{}"),
                ("pages/v1.2/home.wxml", "<view />"),
            ],
        );
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
    }
    #[test]
    fn accepts_component_without_optional_definition() {
        let report = check(
            r#"{"pages":[],"usingComponents":{"simple-card":"components/simple"}}"#,
            &[(
                "components/simple.ink",
                "<template><view /></template><script setup>export default {};</script>",
            )],
        );
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
    }
    #[test]
    fn locates_duplicate_permissions_and_widget_fields() {
        let app = "{\n  \"note\": \"GEOLOCATION\",\n  \"pages\": [],\n  \"permissions\": [\"GEOLOCATION\", \"GEOLOCATION\"],\n  \"widgets\": [\n    {\"path\":\"widgets/one\",\"family\":\"1x1\"},\n    {\"path\":\"widgets/two\",\"family\":\"bad\"},\n    {\"path\":\"widgets/one\",\"family\":\"1x1\"}\n  ]\n}";
        let report = check(app, &[]);
        let duplicate = report
            .diagnostics
            .iter()
            .find(|d| d.code == "AIX007")
            .unwrap();
        assert_eq!((duplicate.path.as_str(), duplicate.line), ("app.json", 4));
        let family = report
            .diagnostics
            .iter()
            .find(|d| d.code == "AIX011")
            .unwrap();
        assert_eq!(family.line, 7);
        let duplicate_path = report
            .diagnostics
            .iter()
            .find(|d| d.code == "AIX009")
            .unwrap();
        assert_eq!(duplicate_path.line, 8);
    }
    #[test]
    fn skips_duplicate_page_validation() {
        let report = check(r#"{"pages":["pages/missing","pages/missing"]}"#, &[]);
        assert_eq!(
            report
                .diagnostics
                .iter()
                .filter(|d| d.code == "AIX009")
                .count(),
            1
        );
        assert_eq!(
            report
                .diagnostics
                .iter()
                .filter(|d| d.code == "AIX016")
                .count(),
            2
        );
    }
    #[test]
    fn keeps_permission_codes_and_invalid_paths_distinct() {
        let report = check(
            r#"{"pages":[42,"../bad"],"permissions":[7,"NOT_REAL"]}"#,
            &[],
        );
        for code in ["AIX006", "AIX037", "AIX039", "AIX040"] {
            assert!(
                report.diagnostics.iter().any(|d| d.code == code),
                "missing {code}"
            );
        }
    }
    #[test]
    fn nested_template_and_script_text_do_not_create_extra_blocks() {
        let report = check(
            r#"{"pages":["pages/home"],"usingComponents":{"slot-card":"components/slot"}}"#,
            &[
                ("pages/home.ink", "<script setup>const sample = '<page><view /></page>'; // <widget>\nexport default {};</script><page><view /></page>"),
                ("components/slot.ink", "<!-- <template> --><template><view title=\"<template>\"><template slot=\"body\"><text /></template></view></template>"),
            ],
        );
        assert!(!report.has_errors(), "{:?}", report.diagnostics);
    }
    #[test]
    fn dynamic_media_type_is_not_assumed_to_be_image() {
        let report = check(
            r#"{"pages":[]}"#,
            &[("app.js", "navigator.mediaLibrary.list({mediaType: kind});")],
        );
        assert!(report.diagnostics.iter().any(|d| d.code == "AIX028"));
        assert!(!report.diagnostics.iter().any(|d| d.code == "AIX027"));
    }
    #[test]
    fn invalid_utf8_entry_is_reported_once() {
        let report = check_source(vec![
            InputFile::new("app.json", br#"{"pages":["pages/home"]}"#),
            InputFile::new("pages/home.ink", vec![0xef, 0xbb, 0xbf, 0xff]),
        ])
        .unwrap();
        assert!(report
            .diagnostics
            .iter()
            .any(|d| d.code == "AIX002" && d.path == "pages/home.ink"));
    }
    #[test]
    fn json_error_after_multibyte_text_is_located_safely() {
        let report = check(
            "{\"pages\":[],\"title\":\"你好\",\"permissions\":[1,]}",
            &[],
        );
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|d| d.code == "AIX001")
            .unwrap();
        assert_eq!(diagnostic.line, 1);
        assert_eq!(diagnostic.column, 43);
    }
    #[test]
    fn script_syntax_error_uses_parser_span() {
        let report = check(
            r#"{"pages":[]}"#,
            &[("broken.js", "const ok = 1;\nconst broken = ;")],
        );
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|d| d.code == "AIX029")
            .unwrap();
        assert_eq!(diagnostic.line, 2);
    }
}
