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

/// Diagnostics and executed check coverage shared by browser and CLI hosts.
#[derive(Debug, Serialize)]
pub struct PreparedCheck {
    pub report: CheckReport,
    pub coverage: CheckCoverage,
}

/// Work actually reached by the native checker, after source collection.
#[derive(Debug, Default, Serialize)]
pub struct CheckCoverage {
    pub text_files: usize,
    pub structure: bool,
    pub scripts: usize,
    pub schemas: usize,
}

pub fn prepare_check_source(files: Vec<InputFile>) -> Result<PreparedCheck> {
    let files: Vec<_> = collector::normalize_inputs(files)?
        .into_iter()
        .filter(|file| !file.path.split('/').any(|part| part == ".aix"))
        .collect();
    let collected = collector::collect_inputs(files, &collector::CollectOptions::default())?;
    let (report, coverage) = check_files_with_coverage(&collected);
    Ok(PreparedCheck { report, coverage })
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
    check_files_with_coverage(files).0
}

fn check_files_with_coverage(files: &[InputFile]) -> (CheckReport, CheckCoverage) {
    let entries: BTreeMap<&str, &[u8]> = files
        .iter()
        .map(|f| (f.path.as_str(), f.data.as_slice()))
        .collect();
    let mut report = CheckReport::default();
    let mut coverage = CheckCoverage::default();
    let mut texts = BTreeMap::<&str, String>::new();
    let mut jsons = BTreeMap::<&str, Value>::new();
    for file in files {
        if !file.path.ends_with(".json") && !is_source(&file.path) {
            continue;
        }
        coverage.text_files += 1;
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
        return (finish(report), coverage);
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
        return (finish(report), coverage);
    };
    coverage.structure = true;
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
                        check_page(
                            path,
                            &entries,
                            &jsons,
                            &texts,
                            &mut report,
                            &mut coverage.schemas,
                        );
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
                        check_markup(
                            path,
                            content,
                            block.body,
                            block.body_span.start,
                            &mut report,
                        );
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
            || path.ends_with(".cjs")
            || path.ends_with(".cts")
        {
            coverage.scripts += 1;
            check_script(
                path,
                content,
                0,
                content,
                is_typescript_source(path),
                &permissions,
                &mut report,
            );
        } else if path.ends_with(".ink") {
            for block in blocks(content, "script") {
                if !block.attributes.contains_key("def") {
                    coverage.scripts += 1;
                    check_script(
                        path,
                        block.body,
                        block.body_span.start,
                        content,
                        block
                            .attributes
                            .get("lang")
                            .is_some_and(|lang| lang == "ts"),
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
    (finish(report), coverage)
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
    [
        ".ink", ".wxml", ".js", ".ts", ".mjs", ".mts", ".cjs", ".cts",
    ]
    .iter()
    .any(|ext| path.ends_with(ext))
}

fn is_typescript_source(path: &str) -> bool {
    [".ts", ".mts", ".cts"]
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

/// Select named blocks from the core tokenizer; rule evaluation stays here.
fn blocks<'a>(source: &'a str, name: &str) -> Vec<aix::xml::SfcBlock<'a>> {
    aix::xml::parse_sfc_blocks(source)
        .into_iter()
        .filter(|block| block.name == name)
        .collect()
}
fn check_markup(path: &str, full: &str, body: &str, offset: usize, report: &mut CheckReport) {
    if let Err(error) = aix::xml::parse_xml(body) {
        report.error(
            "AIX036",
            path,
            full,
            offset,
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
        .filter(|b| b.attributes.contains_key("def"))
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
                defs[0].body_span.start
                    + json_error_offset(defs[0].body, error.line(), error.column()),
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
        .any(|b| b.attributes.contains_key("def"))
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
    schemas: &mut usize,
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
            check_markup(&ink, content, block.body, block.body_span.start, report);
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
            check_page_schema(&def, &ink, content, report, schemas);
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
            check_markup(&markup, content, content, 0, report);
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
            check_page_schema(def, &config, content, report, schemas);
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
fn check_page_schema(
    def: &Value,
    path: &str,
    content: &str,
    report: &mut CheckReport,
    schemas: &mut usize,
) {
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
    let Some(data) = def.pointer("/schema/data").filter(|data| !data.is_null()) else {
        return;
    };
    *schemas += 1;
    let base = if path.ends_with(".ink") {
        blocks(content, "script")
            .into_iter()
            .find(|block| block.attributes.contains_key("def"))
            .map_or(0, |block| block.body_span.start)
    } else {
        0
    };
    // Validate against the selected draft's meta-schema before compiling. In
    // addition to keyword types, compilation catches invalid patterns and refs.
    let result = match jsonschema::meta::try_validate(data) {
        Ok(Ok(())) => jsonschema::validator_for(data)
            .map(|_| ())
            .map_err(|error| (error.instance_path.to_string(), error.to_string())),
        Ok(Err(error)) => Err((error.instance_path.to_string(), error.to_string())),
        Err(error) => Err((String::new(), error.to_string())),
    };
    if let Err((pointer, message)) = result {
        let pointer = format!("/schema/data{pointer}");
        report.error(
            "AIX047",
            path,
            content,
            json_pointer_offset(content, base, &pointer),
            format!("Invalid JSON Schema at {pointer}: {message}"),
        );
    }
}

fn json_pointer_offset(source: &str, mut offset: usize, pointer: &str) -> usize {
    for segment in pointer.split('/').skip(1) {
        let field = segment.replace("~1", "/").replace("~0", "~");
        let next = if source.as_bytes().get(skip_space(source, offset)) == Some(&b'[') {
            field
                .parse::<usize>()
                .ok()
                .and_then(|index| array_items(source, offset).get(index).map(|range| range.0))
        } else {
            object_field(source, offset, &field).map(|(_, start, _)| start)
        };
        let Some(next) = next else { break };
        offset = next;
    }
    offset
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
            .find(|b| b.attributes.contains_key("def"))
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
        check_markup(&ink, content, block.body, block.body_span.start, report);
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
                check_markup(
                    &entry,
                    component_content,
                    block.body,
                    block.body_span.start,
                    report,
                );
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
                check_markup(&markup, markup_content, markup_content, 0, report);
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

    fn assert_codes(files: Vec<InputFile>, expected: &[&str]) {
        let report = check_source(files).unwrap();
        let codes: BTreeSet<_> = report.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(
            codes,
            expected.iter().copied().collect(),
            "{:#?}",
            report.diagnostics
        );
        assert_eq!(
            report.has_errors(),
            report
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error)
        );
    }

    #[test]
    fn validates_all_app_field_shapes_and_entry_paths() {
        for (app, codes) in [
            (r#"{"pages":[],"engine":"invalid"}"#, vec!["AIX005"]),
            (
                r#"{"engine":123,"window":null,"permissions":false,"widgets":{}}"#,
                vec!["AIX005", "AIX008", "AIX010", "AIX034", "AIX038"],
            ),
            (
                r#"{"pages":[1,"","../bad","a.ink"],"permissions":[1,"UNKNOWN","UNKNOWN"]}"#,
                vec!["AIX006", "AIX007", "AIX037", "AIX039", "AIX040"],
            ),
            (
                r#"{"pages":[],"widgets":[null,{}, {"path":"card","family":"bad","placement":"bad","displayName":1,"description":false}]}"#,
                vec!["AIX011", "AIX012", "AIX017", "AIX035", "AIX043", "AIX044"],
            ),
            (
                r#"{"pages":["home","home"],"widgets":[{"path":"home","family":"1x1"}]}"#,
                vec!["AIX009", "AIX016"],
            ),
            (r#"{"pages":[],"usingComponents":[]}"#, vec!["AIX020"]),
        ] {
            assert_codes(vec![InputFile::new("app.json", app)], &codes);
        }
        assert_codes(
            vec![InputFile::new("app.json", b"\xef\xbb\xbf\xff")],
            &["AIX002"],
        );
        assert_codes(vec![], &["AIX003"]);
        let prepared = prepare_check_source(vec![InputFile::new("app.json", b"null")]).unwrap();
        assert!(!prepared.coverage.structure);
        assert!(prepared.report.has_errors());
    }

    #[test]
    fn diagnoses_page_widget_and_component_structure() {
        for (entry, source, codes) in [
            (
                "home",
                "<script def>null</script><page><view /></page><widget></widget>",
                vec!["AIX030", "AIX041"],
            ),
            (
                "home",
                "<script def>{\"schema\":false}</script><page><view</page>",
                vec!["AIX031", "AIX036"],
            ),
            (
                "home",
                "<script def>{bad}</script><page></page><page></page>",
                vec!["AIX014", "AIX015"],
            ),
            (
                "card",
                "<widget></widget><page></page>",
                vec!["AIX013", "AIX046"],
            ),
            (
                "card",
                "<script def>{}</script><widget></widget><widget></widget>",
                vec!["AIX018", "AIX019"],
            ),
        ] {
            let app = if entry == "home" {
                r#"{"pages":["home"]}"#
            } else {
                r#"{"pages":[],"widgets":[{"path":"card","family":"1x1"}]}"#
            };
            assert_codes(
                vec![
                    InputFile::new("app.json", app),
                    InputFile::new(format!("{entry}.ink"), source),
                ],
                &codes,
            );
        }
        assert_codes(
            vec![
                InputFile::new("app.json", br#"{"pages":["home"]}"#),
                InputFile::new("home.json", b"null"),
                InputFile::new("home.wxml", b"<view"),
            ],
            &["AIX030", "AIX036"],
        );
        // Legacy markup must be decoded and validated, just like an Ink body.
        let prepared = prepare_check_source(vec![
            InputFile::new("app.json", br#"{"pages":["home"]}"#),
            InputFile::new("home.json", b"{}"),
            InputFile::new("home.wxml", b"<view />"),
        ])
        .unwrap();
        assert!(!prepared.report.has_errors());
        assert_eq!(prepared.coverage.text_files, 3);
    }

    #[test]
    fn validates_component_references_and_legacy_cycles() {
        assert_codes(
            vec![
                InputFile::new(
                    "app.json",
                    br#"{"pages":[],"usingComponents":{"a-card":"a","b-card":"b"}}"#,
                ),
                InputFile::new("a.ink", b"<script def>{\"component\":true}</script>"),
                InputFile::new("b.json", br#"{"component":false}"#),
                InputFile::new("b.wxml", b"<view />"),
            ],
            &["AIX025", "AIX026"],
        );
        assert_codes(vec![InputFile::new("app.json", br#"{"pages":[],"usingComponents":{"not-string":1,"plain":"@host/view","bad-path":"../bad","missing-entry":"missing","bad-component":"bad","legacy-component":"legacy"}}"#),
            InputFile::new("bad.ink", b"<script def>{\"component\":false}</script><template><view</template>"),
            InputFile::new("legacy.json", br#"{"component":false}"#)],
            &["AIX021", "AIX022", "AIX023", "AIX024", "AIX026", "AIX036", "AIX042", "AIX045"]);
        assert_codes(vec![InputFile::new("app.json", br#"{"pages":[],"usingComponents":{"a-card":"a"}}"#),
            InputFile::new("a.json", br#"{"component":true,"usingComponents":{"b-card":"b"}}"#), InputFile::new("a.wxml", b"<view />"),
            InputFile::new("b.ink", b"<script def>{\"component\":true,\"usingComponents\":{\"a-card\":\"a\"}}</script><template><view /></template>")], &["AIX032"]);
        assert_codes(
            vec![
                InputFile::new("app.json", br#"{"pages":[]}"#),
                InputFile::new("orphan.ink", b"<script def>{\"component\":true}</script>"),
            ],
            &["AIX025"],
        );
    }

    #[test]
    fn checks_direct_and_dynamic_media_constraints_without_guessing() {
        let source = r#"
            navigator.mediaDevices.getUserMedia();
            navigator.mediaDevices.getUserMedia(...args);
            navigator.mediaDevices.getUserMedia(options);
            navigator.mediaDevices.getUserMedia({ ...options, [key]: true });
            navigator.mediaDevices.getUserMedia({audio: false, video: false});
            navigator.mediaDevices.getUserMedia({audio: {}, video: {width: 480}});
            navigator.mediaDevices.getUserMedia({audio: enabled, video: true});
            navigator.mediaDevices.getUserMedia({audio: true});
            navigator.mediaDevices.getUserMedia({unrelated: true});
            navigator.mediaLibrary.list();
            navigator.mediaLibrary.get('id', {});
            navigator.mediaLibrary.list({mediaType: 'audio'});
            navigator.mediaLibrary.list({mediaType: type});
            navigator.mediaLibrary.list({mediaType: 'video'});
            navigator.mediaLibrary.list(options);
            navigator.mediaLibrary.list(...args);
            navigator.mediaLibrary.save(blob);
            navigator.geolocation.watchPosition(() => {});
            new WebSocket('wss://example.com'); new EventSource('/events'); new XMLHttpRequest();
            wx.request({}); wx.uploadFile({}); wx.downloadFile({});
            getObject()[key](); (fetch)('/'); otherCall();
        "#;
        assert_codes(
            vec![
                InputFile::new("app.json", br#"{"pages":[]}"#),
                InputFile::new("main.js", source),
            ],
            &["AIX027", "AIX028", "AIX033"],
        );
        assert_codes(vec![InputFile::new("app.json", br#"{"pages":[],"permissions":["INTERNET","RECORD_AUDIO","CAMERA","READ_MEDIA_IMAGES","READ_MEDIA_AUDIO","GEOLOCATION"]}"#), InputFile::new("main.js", b"fetch('/'); navigator.mediaDevices.getUserMedia({audio:true,video:true}); navigator.mediaLibrary.list({mediaType:'audio'}); navigator.mediaLibrary.get('id'); navigator.geolocation.watchPosition(()=>{});")], &[]);
    }

    #[test]
    fn json_spans_handle_escaped_keys_arrays_and_truncated_input() {
        let source = r#"{"a/b~":[{"quoted":"a\\\"b"},true,null],"last":{}}"#;
        let offset = json_pointer_offset(source, 0, "/a~1b~0/0/quoted");
        assert!(source[offset..].starts_with('"'));
        assert_eq!(json_pointer_offset(source, 0, "/missing"), 0);
        assert!(json_pointer_offset(source, 0, "/a~1b~0/99") > 0);
        assert_eq!(field_offset("text \"field\"", "field"), 5);
        assert_eq!(field_offset("nothing", "field"), 0);
        assert_eq!(component_value_offset("{}", "missing"), 0);
        for malformed in [
            "",
            "{",
            "{\"key\" true}",
            "{\"key\":{} ?}",
            "[{} ?]",
            "[",
            "[\"unfinished",
        ] {
            assert!(value_end(malformed, 0).is_none(), "{malformed}");
        }
        for malformed in [
            "[]",
            "{}",
            "{\"key\" true}",
            "{\"key\":0 ?}",
            "{\"bad\\q\":0}",
        ] {
            assert!(object_field(malformed, 0, "missing").is_none());
        }
        assert!(array_items("{}", 0).is_empty());
        assert!(array_items("[", 0).is_empty());
        assert_eq!(array_items("[{} ?]", 0), vec![(1, 3)]);
        let mut report = CheckReport::default();
        report.error("test", "main.js", "😀\nx", 2, "boundary");
        report.error("test", "main.js", "😀\nx", usize::MAX, "end");
        assert_eq!(
            (report.diagnostics[0].line, report.diagnostics[0].column),
            (1, 1)
        );
        assert_eq!(
            (report.diagnostics[1].line, report.diagnostics[1].column),
            (2, 2)
        );
    }

    #[test]
    fn source_and_archive_agree_on_sfc_boundaries() {
        let definition = r#"{"navigationBarTitleText":"Real title","description":"Page description","schema":{"data":{"type":"object","properties":{"name":{"type":"string"}}}}}"#;
        let closing_definition = definition.replace("Page description", "literal </script> text");
        let page = r#"<page><view class="card" /></page>"#;
        let style = ".card { width: 320px; height: 90px; }";
        let cases = [
            (
                "commented definition",
                format!(
                    r#"<script def>{definition}</script>{page}<style>{style}</style><!-- archived example > <script def>{{"navigationBarTitleText":"COMMENTED OUT","schema":{{"data":{{}}}}}}</script> -->"#
                ),
                "Page description",
            ),
            (
                "nested markup",
                format!(
                    r#"<script def>{definition}</script><page><page><view /></page><view class="card" /></page><style>{style}</style>"#
                ),
                "Page description",
            ),
            (
                "quoted attributes",
                format!(
                    r#"<script data-note="a > b" def>{definition}</script><page data-note="a > b"><view class="card" /></page><style>{style}</style>"#
                ),
                "Page description",
            ),
            (
                "raw script and style text",
                format!(
                    r#"<script setup>const example = "</script><page><view /></page>"; /* </script> */</script><script def>{closing_definition}</script>{page}<style>.example {{ content: "</style>"; }}{style}</style>"#
                ),
                "literal </script> text",
            ),
        ];
        for (name, source, description) in cases {
            let files = vec![
                InputFile::new("app.json", br#"{"pages":["pages/home"]}"#),
                InputFile::new("pages/home.ink", source.as_bytes()),
            ];
            let report = check_source(files.clone()).unwrap();
            assert!(
                report.diagnostics.is_empty(),
                "{name}: {:?}",
                report.diagnostics
            );
            let output = crate::pack(files, crate::PackOptions::new("sfc-regression")).unwrap();
            assert!(output.warnings.is_empty(), "{name}: {:?}", output.warnings);
            let reader = aix::AixReader::new(output.data).unwrap();
            let pages = reader.get_pages();
            assert_eq!(pages.len(), 1, "{name}");
            assert_eq!(pages[0].title.as_deref(), Some("Real title"), "{name}");
            assert_eq!(pages[0].description.as_deref(), Some(description), "{name}");
            assert_eq!(
                pages[0].data_schema["properties"]["name"]["type"], "string",
                "{name}"
            );
            assert_eq!(
                pages[0].size,
                aix::PageConstraint {
                    width: 320.0,
                    height: 90.0
                },
                "{name}"
            );
            assert_eq!(
                aix::PageAnalyzer::analyze_sfc(&source),
                pages[0].size,
                "{name}"
            );
            let tools = reader.get_tools();
            assert_eq!(tools.len(), 1, "{name}");
            assert_eq!(tools[0].target, aix::ToolTarget::Current, "{name}");
            assert_eq!(tools[0].function.parameters, pages[0].data_schema, "{name}");
            assert_eq!(tools[0].layout, pages[0].size, "{name}");
        }
    }

    #[test]
    fn shared_sfc_attributes_keep_script_diagnostics_at_original_positions() {
        let content = "<!-- 😀 > <script def>{invalid}</script> -->\n<page><view /></page>\n<script setup data-note=\"example def text >\" lang = 'ts'>\nconst value: number = ;\n</script>";
        let report = check(
            r#"{"pages":["pages/home"]}"#,
            &[("pages/home.ink", content)],
        );
        assert_eq!(report.diagnostics.len(), 1, "{:?}", report.diagnostics);
        let diagnostic = &report.diagnostics[0];
        assert_eq!(diagnostic.code, "AIX029");
        assert_eq!(diagnostic.path, "pages/home.ink");
        assert_eq!((diagnostic.line, diagnostic.column), (4, 23));
    }

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

    #[test]
    fn validates_nested_page_json_schema_and_source_location() {
        let content = "<script setup>const example = {schema: 1};</script>\n<script def>\n{\n  \"schema\": {\"data\": {\n    \"type\": \"object\",\n    \"properties\": {\"你好/a~b\": {\"type\": \"invalid\"}}\n  }}\n}\n</script><page><view /></page>";
        let report = check(
            r#"{"pages":["pages/home"]}"#,
            &[("pages/home.ink", content)],
        );
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|d| d.code == "AIX047")
            .unwrap();
        assert_eq!(diagnostic.path, "pages/home.ink");
        assert_eq!(diagnostic.line, 6);
        assert!(diagnostic
            .message
            .contains("/schema/data/properties/你好~1a~0b/type"));
        assert_eq!(diagnostic.severity, Severity::Error);
    }

    #[test]
    fn accepts_json_schema_drafts_and_local_recursive_refs() {
        for schema in [
            r#"{"$schema":"http://json-schema.org/draft-07/schema#","type":"object","properties":{"count":{"type":"integer","minimum":0}},"required":["count"]}"#,
            r##"{"$schema":"https://json-schema.org/draft/2020-12/schema","$defs":{"node":{"type":"object","properties":{"next":{"$ref":"#/$defs/node"}}}},"$ref":"#/$defs/node"}"##,
            "{}",
            "true",
            "false",
            "null",
        ] {
            let content = format!("{{\"schema\":{{\"data\":{schema}}}}}");
            let report = check(
                r#"{"pages":["pages/home"]}"#,
                &[
                    ("pages/home.json", &content),
                    ("pages/home.wxml", "<view />"),
                ],
            );
            assert!(!report.has_errors(), "{schema}: {:?}", report.diagnostics);
        }
    }

    #[test]
    fn rejects_invalid_schema_keywords_refs_patterns_and_drafts() {
        for schema in [
            r#"{"type":"object","required":"name"}"#,
            r#"{"type":"array","items":{"minimum":"zero"}}"#,
            r#"{"type":"string","pattern":"["}"#,
            r##"{"$ref":"#/$defs/missing"}"##,
            r#"{"$schema":"https://example.invalid/custom-schema"}"#,
            r#"{"$ref":"https://example.invalid/remote-schema"}"#,
            "42",
        ] {
            let content = format!("{{\"schema\":{{\"data\":{schema}}}}}");
            let report = check(
                r#"{"pages":["pages/home"]}"#,
                &[
                    ("pages/home.json", &content),
                    ("pages/home.wxml", "<view />"),
                ],
            );
            assert!(
                report.diagnostics.iter().any(|d| d.code == "AIX047"),
                "{schema}: {:?}",
                report.diagnostics
            );
        }
    }

    #[test]
    fn checks_commonjs_and_typescript_declaration_syntax() {
        let prepared = prepare_check_source(vec![
            InputFile::new("app.json", br#"{"pages":[]}"#),
            InputFile::new("helper.cjs", b"module.exports = 1;"),
            InputFile::new(
                "main.cts",
                b"import value = require('helper'); export = value;",
            ),
            InputFile::new(
                "types.d.cts",
                b"declare const value: number; export = value;",
            ),
        ])
        .unwrap();
        assert!(
            !prepared.report.has_errors(),
            "{:?}",
            prepared.report.diagnostics
        );
        assert_eq!(prepared.coverage.scripts, 3);
    }

    #[test]
    fn coverage_tracks_collected_and_executed_checks() {
        let prepared = prepare_check_source(vec![
            InputFile::new("app.json", br#"{"pages":["home"]}"#),
            InputFile::new(".aixignore", b"ignored.ts"),
            InputFile::new("ignored.ts", b"const broken = ;"),
            InputFile::new("helper.js", b"export const count = 1;"),
            InputFile::new("home.ink", br#"<script def>{"schema":{"data":{"type":"object"}}}</script><page><view /></page><script lang="ts">const count: number = 1;</script>"#),
        ])
        .unwrap();
        assert!(!prepared.report.has_errors());
        assert_eq!(prepared.coverage.text_files, 3);
        assert!(prepared.coverage.structure);
        assert_eq!(prepared.coverage.scripts, 2);
        assert_eq!(prepared.coverage.schemas, 1);
    }

    #[test]
    fn coverage_does_not_mark_early_return_checks_as_executed() {
        let prepared = prepare_check_source(vec![
            InputFile::new("app.json", b"null"),
            InputFile::new("main.ts", b"const broken = ;"),
        ])
        .unwrap();
        assert!(prepared.report.has_errors());
        assert_eq!(prepared.coverage.text_files, 2);
        assert!(!prepared.coverage.structure);
        assert_eq!(prepared.coverage.scripts, 0);
        assert_eq!(prepared.coverage.schemas, 0);
    }
}
