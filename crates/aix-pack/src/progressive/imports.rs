use super::{resolve_path, AssetKind, Reference};
use anyhow::{anyhow, bail, Context, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use std::collections::BTreeSet;

pub(super) struct Declaration {
    pub source_path: String,
    pub kind: AssetKind,
    pub reference: Reference,
    pub replacement: std::ops::Range<usize>,
    pub dynamic: bool,
}

pub(super) fn scan(
    path: &str,
    source: &str,
    declarations: &mut Vec<Declaration>,
    ordinary: &mut BTreeSet<String>,
) -> Result<()> {
    let mut scan = |script: &str, start: usize, ts: bool| -> Result<()> {
        let allocator = Allocator::default();
        let syntax = if ts {
            SourceType::ts()
        } else {
            SourceType::mjs()
        };
        let parsed = Parser::new(&allocator, script, syntax).parse();
        if let Some(error) = parsed.diagnostics.first() {
            if path.ends_with(".ink") {
                if !script.contains("progressive") {
                    return Ok(());
                }
                bail!("{path}: {error}");
            }
            bail!("Failed to minify {path}: {error}");
        }
        let mut visitor = Imports {
            path,
            source,
            start,
            declarations,
            ordinary,
            error: None,
        };
        visitor.visit_program(&parsed.program);
        visitor.error.map_or(Ok(()), Err)
    };
    if path.ends_with(".ink") {
        for block in aix::xml::parse_sfc_blocks(source) {
            if block.name == "script" && !block.attributes.contains_key("def") {
                scan(
                    block.body,
                    block.body_span.start,
                    block.attributes.get("lang").is_some_and(|v| {
                        v.eq_ignore_ascii_case("ts") || v.eq_ignore_ascii_case("typescript")
                    }),
                )?;
            }
        }
    } else {
        scan(source, 0, path.ends_with(".ts"))?;
    }
    Ok(())
}

struct Imports<'a> {
    path: &'a str,
    source: &'a str,
    start: usize,
    declarations: &'a mut Vec<Declaration>,
    ordinary: &'a mut BTreeSet<String>,
    error: Option<anyhow::Error>,
}

impl Imports<'_> {
    fn location(&self, span: Span) -> Reference {
        let prefix = &self.source[..self.start + span.start as usize];
        Reference {
            path: self.path.to_owned(),
            line: prefix.bytes().filter(|&b| b == b'\n').count() + 1,
            column: prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
        }
    }

    fn mark(
        &mut self,
        source: Option<&str>,
        progressive: Option<&str>,
        kind: Option<&str>,
        span: Span,
        dynamic: bool,
    ) -> Result<()> {
        let reference = self.location(span);
        let result = (|| {
            let Some(progressive) = progressive else {
                if let Some(path) = source.and_then(|v| resolve_path(self.path, v).ok()) {
                    self.ordinary.insert(path);
                }
                return Ok(());
            };
            if progressive != "prefer" {
                bail!("expected progressive: 'prefer'");
            }
            let kind = match kind {
                Some("audio") => AssetKind::Audio,
                Some("video") => AssetKind::Video,
                _ => bail!("progressive import requires type 'audio' or 'video'"),
            };
            let source =
                source.ok_or_else(|| anyhow!("progressive import requires a literal path"))?;
            if !source.starts_with("./") && !source.starts_with("../") {
                bail!("progressive imports require a relative package path");
            }
            let source_path = resolve_path(self.path, source)?;
            self.declarations.push(Declaration {
                source_path,
                kind,
                reference: reference.clone(),
                replacement: self.start + span.start as usize..self.start + span.end as usize,
                dynamic,
            });
            Ok(())
        })();
        result
            .with_context(|| format!("{}:{}:{}", reference.path, reference.line, reference.column))
    }

    fn static_import(&mut self, source: &StringLiteral<'_>, clause: Option<&WithClause<'_>>) {
        let mut progressive = None;
        let mut kind = None;
        if let Some(clause) = clause {
            for entry in &clause.with_entries {
                let key = match &entry.key {
                    ImportAttributeKey::Identifier(key) => key.name.as_str(),
                    ImportAttributeKey::StringLiteral(key) => key.value.as_str(),
                };
                match key {
                    "progressive" => progressive = Some(entry.value.value.as_str()),
                    "type" => kind = Some(entry.value.value.as_str()),
                    _ => {}
                }
            }
            if progressive.is_some()
                && (clause.keyword != WithClauseKeyword::With || clause.with_entries.len() != 2)
            {
                let loc = self.location(clause.span);
                self.error = Some(anyhow!(
                    "{}:{}:{}: expected only type and progressive import attributes",
                    loc.path,
                    loc.line,
                    loc.column
                ));
                return;
            }
        }
        let span = Span::new(
            source.span.start,
            clause.map_or(source.span.end, |v| v.span.end),
        );
        if let Err(error) = self.mark(Some(source.value.as_str()), progressive, kind, span, false) {
            self.error = Some(error);
        }
    }
}

impl<'a> Visit<'a> for Imports<'_> {
    fn visit_import_declaration(&mut self, node: &ImportDeclaration<'a>) {
        if node.import_kind == ImportOrExportKind::Value
            && !node.specifiers.as_ref().is_some_and(|specifiers| !specifiers.is_empty() && specifiers.iter().all(|s| matches!(s, ImportDeclarationSpecifier::ImportSpecifier(s) if s.import_kind == ImportOrExportKind::Type))) {
            self.static_import(&node.source, node.with_clause.as_deref());
        }
    }

    fn visit_export_named_declaration(&mut self, node: &ExportNamedDeclaration<'a>) {
        if node.export_kind == ImportOrExportKind::Value {
            if let Some(source) = &node.source {
                if node.specifiers.is_empty()
                    || node
                        .specifiers
                        .iter()
                        .any(|s| s.export_kind == ImportOrExportKind::Value)
                {
                    self.static_import(source, node.with_clause.as_deref());
                }
            }
        }
        walk::walk_export_named_declaration(self, node);
    }

    fn visit_export_all_declaration(&mut self, node: &ExportAllDeclaration<'a>) {
        if node.export_kind == ImportOrExportKind::Value {
            self.static_import(&node.source, node.with_clause.as_deref());
        }
    }

    fn visit_import_expression(&mut self, node: &ImportExpression<'a>) {
        let source = match &node.source {
            Expression::StringLiteral(value) => Some(value.value.as_str()),
            _ => None,
        };
        let mut progress = None;
        let mut kind = None;
        let mut plain = false;
        if let Some(Expression::ObjectExpression(options)) = &node.options {
            for prop in &options.properties {
                let ObjectPropertyKind::ObjectProperty(prop) = prop else {
                    continue;
                };
                if prop.key.static_name().as_deref() != Some("with") {
                    continue;
                }
                if let Expression::ObjectExpression(attrs) = &prop.value {
                    plain = options.properties.len() == 1
                        && attrs.properties.len() == 2
                        && !prop.computed;
                    for attr in &attrs.properties {
                        let ObjectPropertyKind::ObjectProperty(attr) = attr else {
                            plain = false;
                            continue;
                        };
                        let value = match &attr.value {
                            Expression::StringLiteral(v) => Some(v.value.as_str()),
                            _ => None,
                        };
                        plain &= !attr.computed;
                        match attr.key.static_name().as_deref() {
                            Some("progressive") => progress = Some(value.unwrap_or_default()),
                            Some("type") => kind = value,
                            _ => plain = false,
                        }
                    }
                }
            }
        }
        if progress.is_some() && !plain {
            let loc = self.location(node.span());
            self.error = Some(anyhow!("{}:{}:{}: progressive import options must be a literal with object containing only type and progressive", loc.path, loc.line, loc.column));
        } else if let Err(error) = self.mark(source, progress, kind, node.span(), true) {
            self.error = Some(error);
        }
        walk::walk_import_expression(self, node);
    }
}
