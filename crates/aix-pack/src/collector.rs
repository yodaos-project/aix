//! Shared source collection helpers for directory-style AIX packaging.
//!
//! This module sits above the in-memory [`crate::pack`] engine and is
//! responsible for turning source-side files into normalized [`crate::InputFile`]
//! values before packaging.
//!
//! The collector centralizes behavior that must stay consistent across the
//! native CLI, the npm CLI, and Web/WASM surfaces:
//!
//! - source path normalization
//! - `.aixignore` parsing and matching
//! - exclusion of `.aixignore` from package output
//! - duplicate path detection after normalization
//! - directory reading for native filesystem entry points
//!
//! The low-level packer remains unchanged: callers that already have a prepared
//! `Vec<InputFile>` can continue to call [`crate::pack`] directly. This module
//! is only for callers that want shared "collect then pack" behavior.

use crate::{
    normalize_text_to_utf8, pack, pack_with_progress, InputFile, PackOptions, PackOutput,
    PackProgressEvent,
};
use anyhow::{anyhow, Context, Result};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const RULE_FILE_NAME: &str = ".aixignore";
const VIRTUAL_ROOT: &str = "__aix_source_root__";

/// Options that control how source-side files are collected before packaging.
///
/// These options affect only the collection phase. They do not change the
/// semantics of the underlying AIX packer.
///
/// The default configuration is intended for normal directory packaging:
///
/// - `.aixignore` files are interpreted as ignore-rule inputs
/// - rule files themselves are excluded from the final archive
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CollectOptions {
    /// Excludes `.aixignore` files from the generated package output.
    ///
    /// Even when this is enabled, rule files are still parsed and used during
    /// collection. This only controls whether the rule file itself is emitted
    /// into the final `Vec<InputFile>`.
    pub exclude_rule_file: bool,
}

impl Default for CollectOptions {
    fn default() -> Self {
        Self {
            exclude_rule_file: true,
        }
    }
}

/// Collects source files into normalized package inputs.
///
/// This is the main entry point for callers that already have a source-side
/// file list, but still want shared `.aixignore` handling and path
/// normalization before invoking [`crate::pack`].
///
/// The function performs the following steps:
///
/// 1. Normalize source paths to forward-slash package paths.
/// 2. Reject invalid or duplicate paths after normalization.
/// 3. Build a shared `.aixignore` matcher from any rule files in the input.
/// 4. Exclude ignored files.
/// 5. Optionally exclude `.aixignore` itself from the returned file list.
///
/// Rule files may appear at the root or in nested directories. Nested
/// `.aixignore` files apply to their subtree using `.gitignore`-style
/// semantics via the `ignore` crate.
///
/// The returned files are safe to pass directly to [`crate::pack`].
pub fn collect_inputs(files: Vec<InputFile>, options: &CollectOptions) -> Result<Vec<InputFile>> {
    let mut normalized = normalize_inputs(files)?;
    normalized.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    reject_duplicate_paths(&normalized)?;

    let matchers = build_ignore_matchers(&normalized)?;
    let mut collected = Vec::with_capacity(normalized.len());
    for file in normalized {
        if is_rule_file(&file.path) && options.exclude_rule_file {
            continue;
        }
        if is_ignored(&matchers, &to_virtual_path(&file.path)) {
            continue;
        }
        collected.push(file);
    }
    Ok(collected)
}

/// Collects source files and then forwards them to the existing pack engine.
///
/// This is a convenience wrapper around [`collect_inputs`] followed by
/// [`crate::pack`]. It is useful for callers that want shared collection
/// semantics but do not need to inspect the intermediate normalized file list.
///
/// The returned [`PackOutput`] is identical in shape and meaning to calling
/// [`crate::pack`] directly.
pub fn pack_source_files(
    files: Vec<InputFile>,
    collect_options: &CollectOptions,
    pack_options: PackOptions<'_>,
) -> Result<PackOutput> {
    pack(collect_inputs(files, collect_options)?, pack_options)
}

pub fn pack_source_files_with_progress<F>(
    files: Vec<InputFile>,
    collect_options: &CollectOptions,
    pack_options: PackOptions<'_>,
    mut progress: F,
) -> Result<PackOutput>
where
    F: FnMut(PackProgressEvent) -> Result<()>,
{
    progress(PackProgressEvent::CollectingSourceInputs)?;
    pack_with_progress(
        collect_inputs(files, collect_options)?,
        pack_options,
        progress,
    )
}

/// Reads a native directory tree into source-side [`InputFile`] values.
///
/// This function performs filesystem traversal only. It does **not** apply
/// `.aixignore`, remove rule files, or validate duplicate paths. Use
/// [`pack_directory`] or [`collect_inputs`] for the full shared packaging flow.
///
/// Returned paths are made relative to `root` and normalized to use `/` as the
/// separator so that later collection and packing stages see stable package
/// paths on every platform.
pub fn read_directory(root: &Path) -> Result<Vec<InputFile>> {
    if !root.is_dir() {
        return Err(anyhow!("Input path is not a directory"));
    }

    let root = root.canonicalize()?;
    let mut files = Vec::new();
    read_directory_recursive(&root, &root, &mut files)?;
    Ok(files)
}

/// Reads a native directory, applies shared collection rules, and packages it.
///
/// This is the highest-level native entry point in the collector module and is
/// intended for CLI-style `pack <INPUT_DIR>` flows.
///
/// Behavior includes:
///
/// - recursive directory reading
/// - source path normalization
/// - `.aixignore` evaluation
/// - exclusion of `.aixignore` from the final package by default
/// - forwarding of the collected file set into [`crate::pack`]
///
/// The resulting package bytes and optimization report come from the existing
/// pack engine, so archive semantics remain identical to other pack entry
/// points.
pub fn pack_directory(
    root: &Path,
    collect_options: &CollectOptions,
    pack_options: PackOptions<'_>,
) -> Result<PackOutput> {
    pack_source_files(read_directory(root)?, collect_options, pack_options)
}

fn read_directory_recursive(root: &Path, current: &Path, files: &mut Vec<InputFile>) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if entry.file_name() == ".aix" {
                continue;
            }
            read_directory_recursive(root, &path, files)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .map_err(|error| anyhow!("Failed to relativize {}: {}", path.display(), error))?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        files.push(InputFile::new(relative, fs::read(&path)?));
    }
    Ok(())
}

fn normalize_inputs(files: Vec<InputFile>) -> Result<Vec<InputFile>> {
    files
        .into_iter()
        .map(|file| {
            Ok(InputFile::new(
                normalize_source_path(&file.path)?,
                file.data,
            ))
        })
        .collect()
}

fn normalize_source_path(path: &str) -> Result<String> {
    let path = path.replace('\\', "/");
    let path = path
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    if path.is_empty()
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(anyhow!("invalid source path: {}", path));
    }
    Ok(path)
}

fn reject_duplicate_paths(files: &[InputFile]) -> Result<()> {
    let mut previous: Option<&str> = None;
    for file in files {
        if previous == Some(file.path.as_str()) {
            return Err(anyhow!("duplicate package path: {}", file.path));
        }
        previous = Some(file.path.as_str());
    }
    Ok(())
}

struct IgnoreRule {
    /// Directory (relative to the virtual root) that owns this matcher.
    dir: PathBuf,
    matcher: Gitignore,
}

fn build_ignore_matchers(files: &[InputFile]) -> Result<Vec<IgnoreRule>> {
    let mut rules = Vec::new();
    for file in files.iter().filter(|file| is_rule_file(&file.path)) {
        let (data, _) = normalize_text_to_utf8(&file.path, &file.data)?;
        let content = String::from_utf8(data)
            .with_context(|| format!("Failed to decode {} as UTF-8", file.path))?;

        // A .aixignore must be matched relative to the directory that contains
        // it, not the package root. `GitignoreBuilder::new` anchors the globs
        // to the directory passed here; the per-line `from` argument is only
        // display metadata and does not affect matching.
        let rule_path = to_virtual_path(&file.path);
        let dir = rule_path
            .parent()
            .map(|parent| parent.to_path_buf())
            .ok_or_else(|| anyhow!("Invalid .aixignore path: {}", file.path))?;

        let mut builder = GitignoreBuilder::new(&dir);
        for line in content.lines() {
            builder
                .add_line(None, line)
                .map_err(|error| anyhow!("Invalid .aixignore in {}: {}", file.path, error))?;
        }
        let matcher = builder
            .build()
            .map_err(|error| anyhow!("Failed to build .aixignore matcher: {}", error))?;
        rules.push(IgnoreRule { dir, matcher });
    }

    // Shallowest first so deeper .aixignore files take precedence.
    rules.sort_by_key(|rule| rule.dir.components().count());
    Ok(rules)
}

fn is_ignored(rules: &[IgnoreRule], virtual_path: &Path) -> bool {
    let mut ignored = false;
    for rule in rules {
        // Only apply a rule to files under the directory that owns it, so a
        // nested .aixignore never affects the root or sibling directories.
        if !virtual_path.starts_with(&rule.dir) {
            continue;
        }
        let matched = rule
            .matcher
            .matched_path_or_any_parents(virtual_path, false);
        if matched.is_ignore() {
            ignored = true;
        } else if matched.is_whitelist() {
            ignored = false;
        }
    }
    ignored
}

fn is_rule_file(path: &str) -> bool {
    path == RULE_FILE_NAME || path.ends_with(&format!("/{RULE_FILE_NAME}"))
}

fn to_virtual_path(path: &str) -> PathBuf {
    let mut output = PathBuf::from(VIRTUAL_ROOT);
    for part in path.split('/') {
        output.push(part);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_files_matched_by_root_aixignore() {
        let collected = collect_inputs(
            vec![
                InputFile::new(".aixignore", b"*.tmp\n"),
                InputFile::new("keep.txt", b"keep"),
                InputFile::new("drop.tmp", b"drop"),
            ],
            &CollectOptions::default(),
        )
        .unwrap();

        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].path, "keep.txt");
    }

    #[test]
    fn applies_nested_aixignore_rules() {
        let collected = collect_inputs(
            vec![
                InputFile::new("nested/.aixignore", b"*.tmp\n"),
                InputFile::new("nested/keep.txt", b"keep"),
                InputFile::new("nested/drop.tmp", b"drop"),
            ],
            &CollectOptions::default(),
        )
        .unwrap();

        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].path, "nested/keep.txt");
    }

    #[test]
    fn honors_whitelist_rules() {
        let collected = collect_inputs(
            vec![
                InputFile::new(".aixignore", b"*.tmp\n!keep.tmp\n"),
                InputFile::new("keep.tmp", b"keep"),
                InputFile::new("drop.tmp", b"drop"),
            ],
            &CollectOptions::default(),
        )
        .unwrap();

        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].path, "keep.tmp");
    }

    #[test]
    fn nested_aixignore_does_not_leak_to_root_or_siblings() {
        let collected = collect_inputs(
            vec![
                InputFile::new("nested/.aixignore", b"*.tmp\n"),
                InputFile::new("nested/keep.txt", b"keep"),
                InputFile::new("nested/drop.tmp", b"drop"),
                InputFile::new("root.tmp", b"root"),
                InputFile::new("sibling/drop.tmp", b"sibling"),
            ],
            &CollectOptions::default(),
        )
        .unwrap();

        let paths: Vec<&str> = collected.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["nested/keep.txt", "root.tmp", "sibling/drop.tmp"]
        );
    }

    #[test]
    fn nested_aixignore_anchored_rule_scopes_to_its_directory() {
        let collected = collect_inputs(
            vec![
                InputFile::new("nested/.aixignore", b"/secret.txt\n"),
                InputFile::new("nested/secret.txt", b"nested-secret"),
                InputFile::new("secret.txt", b"root-secret"),
            ],
            &CollectOptions::default(),
        )
        .unwrap();

        let paths: Vec<&str> = collected.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["secret.txt"]);
    }

    #[test]
    fn nested_aixignore_directory_rule_scopes_to_its_directory() {
        let collected = collect_inputs(
            vec![
                InputFile::new("nested/.aixignore", b"dist/\n"),
                InputFile::new("nested/dist/foo.js", b"foo"),
                InputFile::new("other/dist/foo.js", b"other"),
            ],
            &CollectOptions::default(),
        )
        .unwrap();

        let paths: Vec<&str> = collected.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["other/dist/foo.js"]);
    }

    #[test]
    fn nested_aixignore_whitelist_does_not_reinclude_root_file() {
        let collected = collect_inputs(
            vec![
                InputFile::new(".aixignore", b"*.tmp\n"),
                InputFile::new("nested/.aixignore", b"!keep.tmp\n"),
                InputFile::new("nested/keep.tmp", b"nested"),
                InputFile::new("keep.tmp", b"root"),
                InputFile::new("drop.tmp", b"drop"),
            ],
            &CollectOptions::default(),
        )
        .unwrap();

        let paths: Vec<&str> = collected.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["nested/keep.tmp"]);
    }

    #[test]
    fn normalizes_windows_style_paths() {
        let collected = collect_inputs(
            vec![InputFile::new("pages\\index.json", b"{}")],
            &CollectOptions::default(),
        )
        .unwrap();

        assert_eq!(collected[0].path, "pages/index.json");
    }

    #[test]
    fn rejects_duplicate_paths_after_normalization() {
        let error = collect_inputs(
            vec![
                InputFile::new("pages/index.json", b"{}"),
                InputFile::new("pages\\index.json", b"{}"),
            ],
            &CollectOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("duplicate package path"));
    }
}
