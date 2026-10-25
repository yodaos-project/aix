#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{format, string::String, string::ToString, vec::Vec};
use anyhow::{anyhow, Result};
#[cfg(not(feature = "std"))]
use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};
#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
use wasm_bindgen::prelude::*;

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    fn console_warn(s: &str);
}

fn aix_warn(msg: &str) {
    #[cfg(all(feature = "wasm", target_arch = "wasm32"))]
    console_warn(msg);
    log::warn!("{}", msg);
}
use rawzip::{CompressionMethod, ZipArchive, ZipArchiveEntryWayfinder};

pub mod analyzer;
pub mod crypto;
pub mod xml;
pub use analyzer::{PageAnalyzer, PageConstraint};

/// Describes a single archive entry inside an `.aix` package.
///
/// This metadata is collected from the ZIP central directory and is returned by
/// [`AixReader::list`]. It does not require extracting the file contents.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AixEntry {
    /// Normalized archive path of the entry.
    pub name: String,
    /// Uncompressed size of the entry in bytes.
    pub size: u64,
    /// Compressed size of the entry in bytes as stored in the ZIP archive.
    pub compressed_size: u64,
}

/// Caller-controlled budgets for reading an archive.
///
/// Limits are inclusive. `None` means unlimited; [`Default`] preserves the
/// historical policy of [`AixReader::new`]. Set all three limits when accepting
/// untrusted packages. Directory and `META-INF/aix/*` entries count too.
///
/// The total budget covers declared uncompressed bytes across the archive,
/// not cumulative reads or all memory used by the host. The archive input
/// buffer, parsed metadata, and copies retained by callers need separate budgets.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AixReadLimits {
    /// Maximum number of central-directory entries, including directories.
    pub max_entries: Option<u64>,
    /// Maximum declared and extracted bytes for any single entry.
    pub max_entry_bytes: Option<u64>,
    /// Maximum sum of all central-directory uncompressed sizes.
    pub max_total_bytes: Option<u64>,
}

/// Summarizes a page declared by `app.json`.
///
/// The reader derives this structure from the package page list and page-level
/// metadata files, including inferred layout information from templates and
/// styles.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PageInfo {
    /// Logical page path from `app.json`, such as `pages/index/index`.
    pub name: String,
    /// Navigation title declared by the page or inherited from page metadata.
    pub title: Option<String>,
    /// Human-readable description used for tool derivation when available.
    pub description: Option<String>,
    /// JSON Schema fragment describing the page input payload.
    pub data_schema: serde_json::Value,
    /// Inferred page layout constraints used by clients and tool surfaces.
    pub size: PageConstraint,
}

/// A resource failure encountered while resolving page metadata.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MetadataDiagnostic {
    /// Archive path of the missing, unreadable, or malformed resource.
    pub path: String,
    /// Description of the resolution failure.
    pub message: String,
}

impl MetadataDiagnostic {
    fn new(path: &str, message: impl core::fmt::Display) -> Self {
        Self {
            path: path.to_string(),
            message: message.to_string(),
        }
    }
}

/// Resolved pages and tools, together with failures that tolerant getters hide.
///
/// A failed page is omitted from both lists; other pages retain declaration
/// order. An empty `diagnostics` list distinguishes a valid empty result from
/// failed resolution. Widget metadata is resolved separately by `get_widgets`.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MetadataResolution {
    /// Pages whose required resources and metadata could be read.
    pub pages: Vec<PageInfo>,
    /// Tools derived only from the successfully resolved pages.
    pub tools: Vec<Tool>,
    /// One failure per unresolved page, or an `app.json` configuration failure.
    pub diagnostics: Vec<MetadataDiagnostic>,
}

/// Describes a widget entry declared by `app.json`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct WidgetInfo {
    /// Logical widget path without an extension, such as `widgets/clock/index`.
    pub path: String,
    /// Widget family declared by the application, such as `1x1` or `1x2`.
    pub family: String,
    /// Host placement policy. Missing values retain the legacy persistent behavior.
    #[serde(default)]
    pub placement: WidgetPlacement,
    /// User-facing Widget name from `app.json` or a locale overlay.
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    /// Short description from `app.json` or a locale overlay.
    #[serde(default)]
    pub description: Option<String>,
}

/// Describes how the host manages a Widget's placement.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetPlacement {
    #[default]
    Persistent,
    Overlay,
}

/// Represents an OpenAI-style tool derived from a page definition.
///
/// Tools are produced by [`AixReader::get_tools`] and map page metadata into a
/// structure that is convenient for agent runtimes.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tool {
    /// Tool kind. This crate currently emits `"function"`.
    pub r#type: String,
    /// Where the runtime should open the destination page.
    pub target: ToolTarget,
    /// Recommended layout for rendering the target page.
    pub layout: PageConstraint,
    /// Function-like metadata exposed to the runtime.
    pub function: FunctionDefinition,
}

/// Controls how a runtime should open a page-backed tool target.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ToolTarget {
    /// Open the page in the current runtime context.
    #[serde(rename = "_current", alias = "current")]
    Current,
    /// Open the page in a fresh or blank runtime context.
    #[serde(rename = "_blank", alias = "blank")]
    Blank,
}

/// Describes the callable portion of a derived tool.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FunctionDefinition {
    /// Stable tool name, usually matching the page path.
    pub name: String,
    /// Optional natural-language description for the tool.
    pub description: Option<String>,
    /// JSON Schema describing accepted parameters.
    pub parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct AppPagesConfig {
    pub pages: Vec<String>,
}

#[derive(Deserialize)]
struct AppWidgetsConfig {
    #[serde(default)]
    pub widgets: Vec<WidgetInfo>,
}

#[derive(Deserialize)]
struct AppTitleConfig {
    pub window: Option<WindowConfig>,
}

#[derive(Deserialize, Default)]
struct WidgetLocalization {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize, Default)]
struct LocaleConfig {
    #[serde(default)]
    widgets: HashMap<String, WidgetLocalization>,
}

fn normalize_locale(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}

#[derive(Deserialize)]
struct WindowConfig {
    #[serde(rename = "navigationBarTitleText")]
    pub navigation_bar_title_text: Option<String>,
}

#[derive(Deserialize, Debug)]
struct PageConfig {
    #[serde(rename = "navigationBarTitleText")]
    pub navigation_bar_title_text: Option<String>,
    pub description: Option<String>,
    pub schema: Option<PageSchema>,
}

#[derive(Deserialize, Debug)]
struct PageSchema {
    pub data: Option<serde_json::Value>,
}

/// Reads and inspects `.aix` packages from an in-memory byte buffer.
///
/// `AixReader` performs ZIP traversal, entry extraction, manifest access,
/// signature verification, page analysis, and tool derivation without requiring
/// filesystem access.
pub struct AixReader {
    entries: Vec<AixEntry>,
    index: HashMap<String, EntryLocator>,
    data: Vec<u8>,
    limits: AixReadLimits,
}

/// Locates a single archive entry so extraction can seek directly to the
/// local header instead of rescanning the central directory on every read.
#[derive(Clone, Copy)]
struct EntryLocator {
    wayfinder: ZipArchiveEntryWayfinder,
    method: CompressionMethod,
}

/// Checks whether a runtime engine version satisfies a package engine range.
///
/// This is a lightweight convenience wrapper around the engine range utilities
/// in [`crypto`].
///
/// # Parameters
///
/// - `package_engine`: The semver requirement declared by the package.
/// - `runtime_engine`: The concrete engine version provided by the runtime.
///
/// # Returns
///
/// Returns `Ok(true)` when `runtime_engine` matches `package_engine`, `Ok(false)`
/// when it does not, and an error when either input is not a valid version or
/// range.
///
/// # Errors
///
/// Returns an error if the package range or runtime version cannot be parsed.
///
/// # Examples
///
/// ```rust
/// use aiui_aix::satisfy;
///
/// assert!(satisfy("^0.14.0", "0.14.2")?);
/// assert!(!satisfy("^0.14.0", "0.15.0")?);
///
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn satisfy(package_engine: &str, runtime_engine: &str) -> Result<bool> {
    crypto::engine_satisfies(package_engine, runtime_engine)
        .map_err(|error| anyhow!("Invalid engine version or range: {}", error))
}

impl AixReader {
    /// Creates a reader from the raw bytes of an `.aix` archive.
    ///
    /// The constructor scans the ZIP central directory, normalizes entry paths,
    /// and rejects duplicate names before exposing the package through the
    /// reader API.
    /// No caller resource budgets are imposed, for backwards compatibility.
    /// Use [`Self::new_with_limits`] for untrusted packages.
    ///
    /// # Parameters
    ///
    /// - `data`: Full package bytes in ZIP format.
    ///
    /// # Returns
    ///
    /// Returns an initialized reader that can inspect and extract package
    /// contents from memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is not a valid ZIP archive, contains an
    /// invalid path, or includes duplicate normalized entry names.
    pub fn new(data: Vec<u8>) -> Result<Self> {
        Self::new_with_limits(data, AixReadLimits::default())
    }

    /// Creates a reader with caller-controlled archive resource budgets.
    ///
    /// Checks each central-directory entry before indexing it, without
    /// extracting any contents. Repeated reads do not consume the total budget.
    /// Extraction is bounded by both the declared size and the configured byte
    /// budgets, and still verifies CRC and size.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Self::new`], or an error when a budget is exceeded
    /// or the sum of declared sizes overflows `u64`.
    pub fn new_with_limits(data: Vec<u8>, limits: AixReadLimits) -> Result<Self> {
        let mut entries = Vec::new();
        let mut index = HashMap::new();
        let mut total_bytes = 0u64;
        let archive = ZipArchive::from_slice(data.as_slice())
            .map_err(|error| anyhow!("Failed to read zip: {:?}", error))?;
        let mut zip_entries = archive.entries();
        while let Some(file) = zip_entries
            .next_entry()
            .map_err(|error| anyhow!("Failed to read zip entry: {:?}", error))?
        {
            if limits
                .max_entries
                .is_some_and(|max| entries.len() as u64 >= max)
            {
                return Err(anyhow!("Archive entry count limit exceeded"));
            }
            let size = file.uncompressed_size_hint();
            if limits.max_entry_bytes.is_some_and(|max| size > max) {
                return Err(anyhow!("Archive per-entry byte limit exceeded"));
            }
            total_bytes = total_bytes
                .checked_add(size)
                .ok_or_else(|| anyhow!("Archive total declared size overflow"))?;
            if limits.max_total_bytes.is_some_and(|max| total_bytes > max) {
                return Err(anyhow!("Archive total byte limit exceeded"));
            }
            let name = file
                .file_path()
                .try_normalize()
                .map_err(|error| anyhow!("Invalid zip path: {:?}", error))?
                .as_ref()
                .to_string();
            if index.contains_key(&name) {
                return Err(anyhow!("Duplicate zip entry: {}", name));
            }
            index.insert(
                name.clone(),
                EntryLocator {
                    wayfinder: file.wayfinder(),
                    method: file.compression_method(),
                },
            );
            entries.push(AixEntry {
                name,
                size: file.uncompressed_size_hint(),
                compressed_size: file.compressed_size_hint(),
            });
        }

        Ok(Self {
            entries,
            index,
            data,
            limits,
        })
    }

    /// Returns the normalized archive entries contained in the package.
    ///
    /// # Returns
    ///
    /// Returns a borrowed slice describing every entry discovered when the
    /// reader was constructed.
    pub fn list(&self) -> &[AixEntry] {
        &self.entries
    }

    /// Returns the original `.aix` archive bytes borrowed from this reader.
    ///
    /// This is a zero-copy view of the package buffer passed to [`Self::new`].
    /// It is useful when callers need to forward the original archive without
    /// re-reading or re-serializing it.
    ///
    /// # Returns
    ///
    /// Returns a borrowed byte slice containing the exact package bytes used to
    /// construct this reader.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aiui_aix::AixReader;
    /// use std::io::{Cursor, Write};
    /// use zip::write::FileOptions;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut bytes = Vec::new();
    /// {
    ///     let mut zip = zip::ZipWriter::new(Cursor::new(&mut bytes));
    ///     zip.start_file("app.json", FileOptions::default())?;
    ///     zip.write_all(br#"{"pages":[]}"#)?;
    ///     zip.finish()?;
    /// }
    ///
    /// let reader = AixReader::new(bytes.clone())?;
    /// assert_eq!(reader.as_bytes(), bytes.as_slice());
    /// # Ok(())
    /// # }
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        self.data.as_slice()
    }

    /// Consumes the reader and returns the original `.aix` archive bytes.
    ///
    /// Use this when ownership of the underlying package buffer needs to move
    /// out of the reader without cloning.
    ///
    /// # Returns
    ///
    /// Returns the exact byte vector originally supplied to [`Self::new`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aiui_aix::AixReader;
    /// use std::io::{Cursor, Write};
    /// use zip::write::FileOptions;
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut bytes = Vec::new();
    /// {
    ///     let mut zip = zip::ZipWriter::new(Cursor::new(&mut bytes));
    ///     zip.start_file("app.json", FileOptions::default())?;
    ///     zip.write_all(br#"{"pages":[]}"#)?;
    ///     zip.finish()?;
    /// }
    ///
    /// let reader = AixReader::new(bytes.clone())?;
    /// assert_eq!(reader.into_bytes(), bytes);
    /// # Ok(())
    /// # }
    /// ```
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Reads and verifies a single file from the package.
    ///
    /// This method re-opens the in-memory ZIP archive, locates the named entry,
    /// decompresses it when needed, and verifies CRC and uncompressed size
    /// before returning the bytes.
    ///
    /// # Parameters
    ///
    /// - `name`: Normalized archive path of the file to read.
    ///
    /// # Returns
    ///
    /// Returns the extracted file contents as a new byte vector.
    ///
    /// # Errors
    ///
    /// Returns an error if the archive cannot be read, the path is invalid, the
    /// file is missing, the compression method is unsupported, extraction fails,
    /// or ZIP integrity verification does not match the stored metadata.
    pub fn read_file(&self, name: &str) -> Result<Vec<u8>> {
        let locator = self
            .index
            .get(name)
            .copied()
            .ok_or_else(|| anyhow!("File not found: {}", name))?;
        let archive = ZipArchive::from_slice(self.data.as_slice())
            .map_err(|error| anyhow!("Failed to read zip: {:?}", error))?;
        let local = archive
            .get_entry(locator.wayfinder)
            .map_err(|error| anyhow!("Failed to locate {}: {:?}", name, error))?;
        let claimed = local.claim_verifier();
        let limit = claimed
            .uncompressed_size
            .min(self.limits.max_entry_bytes.unwrap_or(u64::MAX))
            .min(self.limits.max_total_bytes.unwrap_or(u64::MAX));
        let limit = usize::try_from(limit).map_err(|_| anyhow!("File too large: {}", name))?;
        let output = match locator.method {
            CompressionMethod::STORE => {
                // A forged uncompressed size must not permit an oversized copy.
                if local.data().len() > limit {
                    return Err(anyhow!("Extracted file size limit exceeded: {}", name));
                }
                local.data().to_vec()
            }
            CompressionMethod::DEFLATE => {
                miniz_oxide::inflate::decompress_to_vec_with_limit(local.data(), limit)
                    .map_err(|error| anyhow!("Failed to extract {}: {}", name, error))?
            }
            _ => {
                return Err(anyhow!(
                    "Unsupported compression for {}: {}",
                    name,
                    locator.method
                ))
            }
        };
        let actual = rawzip::ZipVerification {
            crc: rawzip::crc32(&output),
            uncompressed_size: output.len() as u64,
        };
        claimed
            .valid(actual)
            .map_err(|error| anyhow!("Failed to verify {}: {:?}", name, error))?;
        Ok(output)
    }

    /// Reads the package build identifier from the `VERSION` entry.
    ///
    /// # Returns
    ///
    /// Returns the UTF-8 version string when the entry exists and contains
    /// valid UTF-8. Returns `None` for missing or invalid data.
    pub fn get_version(&self) -> Option<String> {
        self.read_file("VERSION")
            .ok()
            .and_then(|v| String::from_utf8(v).ok())
    }

    fn read_app_config<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        let app_json = self.read_file("app.json")?;
        if !starts_json_object(&app_json) {
            return Err(anyhow!("Invalid app.json: expected an object"));
        }
        serde_json::from_slice(&app_json).map_err(|error| anyhow!("Invalid app.json: {}", error))
    }

    /// Reads the package manifest from `META-INF/aix/manifest.json`.
    ///
    /// Older or unsigned packages may not contain a manifest, so this method
    /// returns `Ok(None)` in that case.
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(...))` when the manifest exists and can be parsed,
    /// `Ok(None)` when the entry is absent, and an error when the manifest entry
    /// exists but is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest file cannot be read or parsed as JSON.
    pub fn get_manifest(&self) -> Result<Option<crypto::PackageManifest>> {
        if !self
            .entries
            .iter()
            .any(|entry| entry.name == crypto::MANIFEST_PATH)
        {
            return Ok(None);
        }

        let data = self.read_file(crypto::MANIFEST_PATH)?;
        serde_json::from_slice(&data)
            .map(Some)
            .map_err(|error| anyhow!("Invalid AIX manifest: {}", error))
    }

    /// Returns the stable project identity, or `None` for legacy packages.
    /// A present but malformed manifest or identity is an error.
    pub fn get_agent_id(&self) -> Result<Option<String>> {
        let id = self.get_manifest()?.and_then(|manifest| manifest.agent_id);
        if id
            .as_deref()
            .is_some_and(|id| !crypto::is_valid_agent_id(id))
        {
            return Err(anyhow!("Invalid AIX manifest agent_id"));
        }
        Ok(id)
    }

    /// Returns the engine range declared by this package manifest.
    ///
    /// # Returns
    ///
    /// Returns the manifest `engine` field when a readable manifest is present.
    /// Returns `None` when the manifest is missing or invalid.
    pub fn get_engine(&self) -> Option<String> {
        self.get_manifest()
            .ok()
            .and_then(|manifest| manifest.map(|manifest| manifest.engine))
    }

    /// Checks whether the package manifest accepts the supplied runtime engine.
    ///
    /// # Parameters
    ///
    /// - `current_version`: Concrete runtime engine version to test.
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` when the runtime version satisfies the manifest
    /// engine range and `Ok(false)` when it does not.
    ///
    /// # Errors
    ///
    /// Returns an error if the package manifest is missing or if either the
    /// declared range or supplied version is invalid.
    pub fn supports_engine(&self, current_version: &str) -> Result<bool> {
        let engine = self
            .get_engine()
            .ok_or_else(|| anyhow!("AIX manifest not found"))?;
        satisfy(&engine, current_version)
    }

    /// Verifies the package manifest signature and every signed package entry.
    ///
    /// The verification flow checks the trusted key identity, validates the
    /// signed manifest, confirms manifest ordering rules, re-hashes every signed
    /// payload entry, ensures no unsigned non-metadata entries are present, and
    /// checks that manifest metadata matches the package contents.
    ///
    /// # Parameters
    ///
    /// - `trusted_key`: Public key that the caller trusts for this package.
    ///
    /// # Returns
    ///
    /// Returns a summary report describing the verified package metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the manifest, signature, or package contents are
    /// missing, malformed, unsigned, out of order, hashed incorrectly, or
    /// signed by a different key.
    pub fn verify_signature(
        &self,
        trusted_key: &crypto::PublicKey,
    ) -> Result<crypto::VerificationReport> {
        let manifest_data = self.read_file(crypto::MANIFEST_PATH)?;
        let manifest: crypto::PackageManifest = serde_json::from_slice(&manifest_data)
            .map_err(|error| anyhow!("Invalid AIX manifest: {}", error))?;
        if manifest.format != "aix"
            || manifest.algorithm != "ed25519"
            || manifest.digest != "sha256"
        {
            return Err(anyhow!("Unsupported AIX signature manifest"));
        }
        crypto::validate_engine_range(&manifest.engine)
            .map_err(|error| anyhow!("Invalid engine range: {}", error))?;
        if manifest.key_id != trusted_key.key_id() {
            return Err(anyhow!("Manifest key ID does not match trusted key"));
        }

        let signature_data = self.read_file(crypto::SIGNATURE_PATH)?;
        let signature_bytes: [u8; 64] = signature_data
            .try_into()
            .map_err(|_| anyhow!("Invalid Ed25519 signature length"))?;
        let signature = crypto::Signature::from_bytes(signature_bytes);
        trusted_key
            .verify(b"package-manifest", &manifest_data, &signature)
            .map_err(|error| anyhow!("AIX signature verification failed: {}", error))?;

        let mut previous: Option<&str> = None;
        for entry in &manifest.entries {
            if previous.is_some_and(|path| path.as_bytes() >= entry.path.as_bytes())
                || entry.path.starts_with("META-INF/aix/")
            {
                return Err(anyhow!(
                    "Invalid or unsorted manifest entry: {}",
                    entry.path
                ));
            }
            let data = self.read_file(&entry.path)?;
            if data.len() as u64 != entry.size || crypto::sha256(&data) != entry.sha256 {
                return Err(anyhow!("Package entry digest mismatch: {}", entry.path));
            }
            previous = Some(&entry.path);
        }
        let signed_paths: HashSet<&str> = manifest
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect();
        for entry in &self.entries {
            if entry.name.ends_with('/') || entry.name.starts_with(crypto::METADATA_PREFIX) {
                continue;
            }
            if !signed_paths.contains(entry.name.as_str()) {
                return Err(anyhow!("Unsigned package entry: {}", entry.name));
            }
        }
        let version = self
            .get_version()
            .ok_or_else(|| anyhow!("VERSION entry not found"))?;
        if manifest.version != version {
            return Err(anyhow!("Manifest version does not match VERSION"));
        }
        if crypto::calculate_package_id(&manifest.entries) != manifest.package_id {
            return Err(anyhow!("Manifest package ID mismatch"));
        }

        Ok(crypto::VerificationReport {
            package_id: manifest.package_id,
            version: manifest.version,
            engine: manifest.engine,
            key_id: manifest.key_id,
            entry_count: manifest.entries.len(),
        })
    }

    /// Returns the application title from `app.json`.
    ///
    /// # Returns
    ///
    /// Returns the `window.navigationBarTitleText` value when `app.json` exists,
    /// parses correctly, and declares a title. Returns `None` otherwise.
    pub fn get_title(&self) -> Option<String> {
        let config: AppTitleConfig = self.read_app_config().ok()?;
        config.window.and_then(|w| w.navigation_bar_title_text)
    }

    /// Resolves pages with the historical tolerant policy.
    ///
    /// Invalid `app.json.pages` returns an empty list. Missing resources and
    /// malformed page metadata retain defaults and may still produce tools.
    /// Widget and title configuration are parsed independently. Use
    /// [`Self::resolve_metadata`] to observe failures and exclude unresolved pages.
    pub fn get_pages(&self) -> Vec<PageInfo> {
        let Ok(config) = self.read_app_config::<AppPagesConfig>() else {
            return Vec::new();
        };
        config
            .pages
            .into_iter()
            .filter_map(|path| self.resolve_page(&path, false).ok())
            .collect()
    }

    /// Resolves page metadata and tools with resource diagnostics.
    ///
    /// Requires a readable `.ink` entry, or both `.json` and `.wxml` entries for
    /// a traditional page. Present metadata/styles must be readable UTF-8;
    /// page JSON and complete Ink `<script def>` blocks must deserialize.
    /// Missing optional schemas, definitions, and styles retain legacy defaults.
    /// A broken Ink entry never falls back to traditional files in this API.
    ///
    /// Failed pages are omitted from `pages` and `tools`, with one diagnostic
    /// per page; other pages remain available in declaration order. An unreadable
    /// or invalid `app.json.pages` produces an `app.json` diagnostic. Widget and
    /// title fields are outside page resolution and cannot hide valid pages.
    ///
    /// This checks resource/metadata resolution, not full Ink syntax, JSON Schema
    /// validity, or runtime execution. Use source checking for those static rules.
    pub fn resolve_metadata(&self) -> MetadataResolution {
        let mut result = MetadataResolution::default();
        match self.read_app_config::<AppPagesConfig>() {
            Ok(config) => {
                for path in config.pages {
                    match self.resolve_page(&path, true) {
                        Ok(page) => result.pages.push(page),
                        Err(diagnostic) => result.diagnostics.push(diagnostic),
                    }
                }
            }
            Err(error) => result
                .diagnostics
                .push(MetadataDiagnostic::new("app.json", error)),
        }
        result.tools = tools_from_pages(result.pages.clone());
        result
    }

    fn read_page_text(
        &self,
        path: &str,
        strict: bool,
    ) -> core::result::Result<Option<String>, MetadataDiagnostic> {
        if !self.index.contains_key(path) {
            return Ok(None);
        }
        let result = self.read_file(path).and_then(|bytes| {
            String::from_utf8(bytes).map_err(|error| anyhow!("Invalid UTF-8: {}", error))
        });
        match result {
            Ok(text) => Ok(Some(text)),
            Err(error) if strict => Err(MetadataDiagnostic::new(path, error)),
            Err(_) => Ok(None),
        }
    }

    fn resolve_page(
        &self,
        path: &str,
        strict: bool,
    ) -> core::result::Result<PageInfo, MetadataDiagnostic> {
        let mut page = PageInfo {
            name: path.to_string(),
            title: None,
            description: None,
            data_schema: serde_json::json!({}),
            size: PageConstraint::default(),
        };
        let ink_path = format!("{}.ink", path);
        if let Some(content) = self.read_page_text(&ink_path, strict)? {
            let blocks = xml::parse_sfc_blocks(&content);
            for block in &blocks {
                if block.name == "script" && block.attributes.contains_key("def") {
                    apply_page_config(&mut page, block.body, &ink_path, strict)?;
                }
            }
            page.size = PageAnalyzer::analyze_sfc_blocks(&blocks);
        } else {
            let json_path = format!("{}.json", path);
            if let Some(content) = self.read_page_text(&json_path, strict)? {
                apply_page_config(&mut page, &content, &json_path, strict)?;
            } else if strict {
                return Err(MetadataDiagnostic::new(
                    &json_path,
                    "Page config file is missing (no Ink entry)",
                ));
            }
            let markup_path = format!("{}.wxml", path);
            let markup = self.read_page_text(&markup_path, strict)?;
            if strict && markup.is_none() {
                return Err(MetadataDiagnostic::new(
                    &markup_path,
                    "Page markup file is missing",
                ));
            }
            let style = if strict {
                match self.read_page_text(&format!("{}.wcss", path), true)? {
                    Some(style) => Some(style),
                    None => self.read_page_text(&format!("{}.wxss", path), true)?,
                }
            } else {
                // Preserve the legacy fallback on read errors, before UTF-8 decoding.
                self.read_file(&format!("{}.wcss", path))
                    .or_else(|_| self.read_file(&format!("{}.wxss", path)))
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
            };
            if let Some(markup) = markup {
                page.size = PageAnalyzer::analyze(Some(&markup), style.as_deref());
            }
        }
        Ok(page)
    }

    /// Returns the widgets declared in `app.json` after validating their entry files.
    ///
    /// Every widget path must resolve to an `.ink` single-file entry in the package.
    /// An absent `widgets` property is treated as an empty list.
    pub fn get_widgets(&self) -> Result<Vec<WidgetInfo>> {
        self.get_widgets_with_locale(None)
    }

    /// Returns widgets with localized metadata for the requested locale.
    ///
    /// The reader selects an `app.<locale>.json` overlay and merges its
    /// `displayName` and `description` values by widget path. Missing overlays
    /// and fields retain their values from `app.json`.
    pub fn get_widgets_for_locale(&self, locale: &str) -> Result<Vec<WidgetInfo>> {
        self.get_widgets_with_locale(Some(locale))
    }

    fn get_widgets_with_locale(&self, locale: Option<&str>) -> Result<Vec<WidgetInfo>> {
        let config: AppWidgetsConfig = self.read_app_config()?;
        let mut widgets = config.widgets;
        if let Some(locale) = locale.filter(|value| !value.trim().is_empty()) {
            if let Some(locale_path) = self.find_locale_file(locale) {
                let locale_config: LocaleConfig =
                    serde_json::from_slice(&self.read_file(&locale_path)?).map_err(|error| {
                        anyhow!("Invalid locale configuration {}: {}", locale_path, error)
                    })?;
                for widget in &mut widgets {
                    if let Some(localized) = locale_config.widgets.get(&widget.path) {
                        if localized.display_name.is_some() {
                            widget.display_name = localized.display_name.clone();
                        }
                        if localized.description.is_some() {
                            widget.description = localized.description.clone();
                        }
                    }
                }
            }
        }
        for widget in &widgets {
            let entry_path = format!("{}.ink", widget.path);
            if !self.index.contains_key(entry_path.as_str()) {
                return Err(anyhow!("Widget entry not found: {}", entry_path));
            }
        }
        Ok(widgets)
    }

    fn find_locale_file(&self, requested: &str) -> Option<String> {
        let requested = normalize_locale(requested);
        if requested.is_empty() {
            return None;
        }

        let mut files = HashMap::new();
        for name in self.index.keys() {
            let Some(locale) = name
                .strip_prefix("app.")
                .and_then(|value| value.strip_suffix(".json"))
            else {
                continue;
            };
            if !locale.is_empty() {
                files.insert(normalize_locale(locale), name.clone());
            }
        }

        let mut candidate = requested.clone();
        while !candidate.is_empty() {
            if let Some(path) = files.get(&candidate) {
                return Some(path.clone());
            }
            candidate = candidate
                .rsplit_once('-')
                .map(|(parent, _)| parent.to_string())
                .unwrap_or_default();
        }

        let primary = requested.split('-').next().unwrap_or_default();
        files
            .into_iter()
            .filter(|(locale, _)| locale.split('-').next() == Some(primary))
            .min_by(|left, right| left.0.cmp(&right.0))
            .map(|(_, path)| path)
    }

    /// Derives agent-facing tools from the package page list.
    ///
    /// The first page without parameters is exposed as [`ToolTarget::Blank`],
    /// while every other page is emitted as [`ToolTarget::Current`]. Parameter
    /// schemas and layout hints are taken from [`Self::get_pages`].
    ///
    /// # Returns
    ///
    /// Returns a list of OpenAI-style tool definitions derived from package
    /// pages.
    pub fn get_tools(&self) -> Vec<Tool> {
        tools_from_pages(self.get_pages())
    }
}

fn apply_page_config(
    page: &mut PageInfo,
    content: &str,
    resource: &str,
    strict: bool,
) -> core::result::Result<(), MetadataDiagnostic> {
    if strict && !starts_json_object(content.as_bytes()) {
        return Err(MetadataDiagnostic::new(
            resource,
            "Invalid page metadata: expected an object",
        ));
    }
    match serde_json::from_str::<PageConfig>(content) {
        Ok(config) => {
            page.title = config.navigation_bar_title_text;
            page.description = config.description;
            match config.schema {
                Some(schema) => match schema.data {
                    Some(data) => page.data_schema = data,
                    None => aix_warn(&format!("Missing 'data' in schema for page: {}", page.name)),
                },
                None => aix_warn(&format!("Missing 'schema' for page: {}", page.name)),
            }
            Ok(())
        }
        Err(error) if strict => Err(MetadataDiagnostic::new(
            resource,
            format!("Invalid page metadata: {}", error),
        )),
        Err(_) => Ok(()),
    }
}

fn starts_json_object(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b'{')
}

fn tools_from_pages(pages: Vec<PageInfo>) -> Vec<Tool> {
    let mut tools = Vec::new();

    for (index, page) in pages.into_iter().enumerate() {
        let has_parameters = page_has_parameters(&page.data_schema);
        let name = page.name;
        let description = page.description.or(page.title);
        let layout = page.size;
        let parameters = page.data_schema;

        let current_tool = Tool {
            r#type: "function".to_string(),
            target: ToolTarget::Current,
            layout,
            function: FunctionDefinition {
                name: name.clone(),
                description: description.clone(),
                parameters,
            },
        };

        if index == 0 {
            if has_parameters {
                tools.push(current_tool);
            } else {
                tools.push(Tool {
                    r#type: "function".to_string(),
                    target: ToolTarget::Blank,
                    layout,
                    function: FunctionDefinition {
                        name,
                        description,
                        parameters: serde_json::json!({}),
                    },
                });
            }
        } else {
            tools.push(current_tool);
        }
    }

    tools
}

fn page_has_parameters(data_schema: &serde_json::Value) -> bool {
    if data_schema.is_null() {
        return false;
    }

    let Some(map) = data_schema.as_object() else {
        return true;
    };

    if map.is_empty() {
        return false;
    }

    let is_empty_object_schema = matches!(
        map.get("type").and_then(serde_json::Value::as_str),
        Some("object")
    ) && matches!(map.get("properties"), Some(serde_json::Value::Object(properties)) if properties.is_empty());

    !is_empty_object_schema
}

/// Formats a byte count using human-readable binary units.
///
/// # Parameters
///
/// - `bytes`: Raw byte count to display.
///
/// # Returns
///
/// Returns a string formatted in bytes, KB, MB, or GB with two decimal places
/// for values larger than one kilobyte.
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::write::FileOptions;

    fn archive(entries: Vec<(&str, Vec<u8>)>) -> AixReader {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in entries {
            zip.start_file(name, FileOptions::default()).unwrap();
            zip.write_all(&data).unwrap();
        }
        AixReader::new(zip.finish().unwrap().into_inner()).unwrap()
    }

    fn limited_archive_error(data: Vec<u8>, limits: AixReadLimits) -> String {
        AixReader::new_with_limits(data, limits)
            .err()
            .unwrap()
            .to_string()
    }

    fn entry_fixture(method: zip::CompressionMethod) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("payload", FileOptions::default().compression_method(method))
            .unwrap();
        zip.write_all(&[42; 4096]).unwrap();
        zip.finish().unwrap().into_inner()
    }

    fn central_offset(data: &[u8]) -> usize {
        data.windows(4).position(|v| v == b"PK\x01\x02").unwrap()
    }

    #[test]
    fn metadata_resolution_keeps_valid_pages_but_excludes_missing_page_tools() {
        let reader = archive(vec![
            ("app.json", br#"{"pages":["missing","good","detail"]}"#.to_vec()),
            ("good.ink", br#"<script def>{"navigationBarTitleText":"Good"}</script><page><view style="width:120px;height:80px" /></page>"#.to_vec()),
            ("detail.json", br#"{"schema":{"data":{"type":"object","properties":{"id":{"type":"string"}}}}}"#.to_vec()),
            ("detail.wxml", b"<view />".to_vec()),
        ]);
        // The existing API retains its tolerant default entry and tool.
        assert_eq!(reader.get_pages().len(), 3);
        assert_eq!(reader.get_tools()[0].function.name, "missing");
        let report = reader.resolve_metadata();
        assert_eq!(
            report
                .pages
                .iter()
                .map(|page| page.name.as_str())
                .collect::<Vec<_>>(),
            ["good", "detail"]
        );
        assert_eq!(
            report
                .tools
                .iter()
                .map(|tool| tool.function.name.as_str())
                .collect::<Vec<_>>(),
            ["good", "detail"]
        );
        assert_eq!(report.tools[0].target, ToolTarget::Blank);
        assert_eq!(report.tools[1].target, ToolTarget::Current);
        assert_eq!(report.pages[0].title.as_deref(), Some("Good"));
        assert_eq!(report.pages[0].size.width, 120.0);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].path, "missing.json");
        assert!(report.diagnostics[0].message.contains("missing"));
    }

    #[test]
    fn page_widget_and_title_configurations_do_not_hide_each_other() {
        for unrelated in [
            r#""widgets":[{"path":"card","family":"1x1","placement":"invalid"}]"#,
            r#""widgets":false"#,
            r#""window":false"#,
        ] {
            let reader = archive(vec![
                (
                    "app.json",
                    format!(r#"{{"pages":["good"],{unrelated}}}"#).into_bytes(),
                ),
                ("good.ink", b"<page><view /></page>".to_vec()),
            ]);
            assert_eq!(reader.get_pages().len(), 1);
            assert_eq!(reader.get_tools().len(), 1);
            let report = reader.resolve_metadata();
            assert_eq!(report.pages.len(), 1);
            assert!(report.diagnostics.is_empty());
            if unrelated.contains("widgets") {
                assert!(reader
                    .get_widgets()
                    .unwrap_err()
                    .to_string()
                    .contains("Invalid app.json"));
            }
        }
        let reader = archive(vec![
            ("app.json", br#"{"pages":false,"widgets":[{"path":"card","family":"1x1"}],"window":{"navigationBarTitleText":"Title"}}"#.to_vec()),
            ("card.ink", b"<widget></widget>".to_vec()),
        ]);
        assert_eq!(reader.get_title().as_deref(), Some("Title"));
        assert_eq!(reader.get_widgets().unwrap().len(), 1);
        assert!(reader.get_pages().is_empty());
        assert_eq!(reader.resolve_metadata().diagnostics[0].path, "app.json");
    }

    #[test]
    fn metadata_resolution_distinguishes_empty_missing_and_invalid_page_lists() {
        assert!(archive(vec![("app.json", br#"{"pages":[]}"#.to_vec())])
            .resolve_metadata()
            .diagnostics
            .is_empty());
        for entries in [
            vec![],
            vec![("app.json", b"invalid".to_vec())],
            vec![("app.json", b"{}".to_vec())],
            vec![("app.json", b"[[]]".to_vec())],
            vec![("app.json", br#"{"pages":[1]}"#.to_vec())],
        ] {
            let report = archive(entries).resolve_metadata();
            assert!(report.pages.is_empty());
            assert!(report.tools.is_empty());
            assert_eq!(report.diagnostics.len(), 1);
            assert_eq!(report.diagnostics[0].path, "app.json");
        }
    }

    #[test]
    fn metadata_resolution_reports_missing_or_malformed_traditional_resources() {
        for (resources, expected_path, message) in [
            (
                vec![("home.wxml", b"<view />".to_vec())],
                "home.json",
                "missing",
            ),
            (vec![("home.json", b"{}".to_vec())], "home.wxml", "missing"),
            (
                vec![("home.json", b"{bad".to_vec())],
                "home.json",
                "Invalid page metadata",
            ),
            (
                vec![("home.json", b"[null,null,null]".to_vec())],
                "home.json",
                "expected an object",
            ),
            (
                vec![("home.json", br#"{"schema":false}"#.to_vec())],
                "home.json",
                "Invalid page metadata",
            ),
            (
                vec![("home.json", b"{}".to_vec()), ("home.wxml", vec![255])],
                "home.wxml",
                "Invalid UTF-8",
            ),
            (
                vec![
                    ("home.json", b"{}".to_vec()),
                    ("home.wxml", b"<view />".to_vec()),
                    ("home.wcss", vec![255]),
                ],
                "home.wcss",
                "Invalid UTF-8",
            ),
            (
                vec![
                    ("home.json", b"{}".to_vec()),
                    ("home.wxml", b"<view />".to_vec()),
                    ("home.wxss", vec![255]),
                ],
                "home.wxss",
                "Invalid UTF-8",
            ),
        ] {
            let mut entries = vec![("app.json", br#"{"pages":["home"]}"#.to_vec())];
            entries.extend(resources);
            let reader = archive(entries);
            assert_eq!(reader.get_pages().len(), 1);
            let report = reader.resolve_metadata();
            assert!(report.pages.is_empty());
            assert!(report.tools.is_empty());
            assert_eq!(report.diagnostics[0].path, expected_path);
            assert!(report.diagnostics[0].message.contains(message));
        }
    }

    #[test]
    fn metadata_resolution_does_not_fall_back_from_a_broken_ink_entry() {
        for body in [
            vec![255],
            b"<script def>{bad}</script><page><view /></page>".to_vec(),
        ] {
            let reader = archive(vec![
                ("app.json", br#"{"pages":["home"]}"#.to_vec()),
                ("home.ink", body),
                ("home.json", b"{}".to_vec()),
                ("home.wxml", b"<view />".to_vec()),
            ]);
            assert_eq!(reader.get_pages().len(), 1);
            let report = reader.resolve_metadata();
            assert!(report.pages.is_empty());
            assert_eq!(report.diagnostics[0].path, "home.ink");
        }
    }

    #[test]
    fn metadata_resolution_preserves_optional_defaults_and_css_aliases() {
        for config in ["{}", r#"{"schema":{}}"#] {
            for style in [None, Some("home.wcss"), Some("home.wxss")] {
                let mut entries = vec![
                    ("app.json", br#"{"pages":["home"]}"#.to_vec()),
                    ("home.json", config.as_bytes().to_vec()),
                    ("home.wxml", b"<view class='card' />".to_vec()),
                ];
                if let Some(style) = style {
                    entries.push((style, b".card { width: 160px; height: 80px; }".to_vec()));
                }
                let reader = archive(entries);
                let report = reader.resolve_metadata();
                assert!(report.diagnostics.is_empty());
                assert_eq!(
                    serde_json::to_value(&report.pages).unwrap(),
                    serde_json::to_value(reader.get_pages()).unwrap()
                );
                assert_eq!(
                    serde_json::to_value(&report.tools).unwrap(),
                    serde_json::to_value(reader.get_tools()).unwrap()
                );
                if style.is_some() {
                    assert_eq!(report.pages[0].size.width, 160.0);
                }
            }
        }
        let report = archive(vec![
            ("app.json", br#"{"pages":["home"]}"#.to_vec()),
            ("home.ink", b"<page><view /></page>".to_vec()),
        ])
        .resolve_metadata();
        assert!(report.diagnostics.is_empty());
        assert_eq!(report.pages[0].data_schema, serde_json::json!({}));
    }

    #[test]
    fn metadata_resolution_propagates_crc_failures_instead_of_defaults() {
        for target in ["app.json", "home.ink"] {
            let mut data = archive(vec![
                ("app.json", br#"{"pages":["home"]}"#.to_vec()),
                ("home.ink", b"<page><view /></page>".to_vec()),
            ])
            .into_bytes();
            let archive = ZipArchive::from_slice(data.as_slice()).unwrap();
            let mut entries = archive.entries();
            let mut offset = central_offset(&data);
            while let Some(entry) = entries.next_entry().unwrap() {
                if entry.file_path().as_ref() == target.as_bytes() {
                    break;
                }
                let name_len =
                    u16::from_le_bytes(data[offset + 28..offset + 30].try_into().unwrap()) as usize;
                let extra_len =
                    u16::from_le_bytes(data[offset + 30..offset + 32].try_into().unwrap()) as usize;
                let comment_len =
                    u16::from_le_bytes(data[offset + 32..offset + 34].try_into().unwrap()) as usize;
                offset += 46 + name_len + extra_len + comment_len;
            }
            data[offset + 16] ^= 1;
            let report = AixReader::new(data).unwrap().resolve_metadata();
            assert!(report.pages.is_empty());
            assert_eq!(report.diagnostics[0].path, target);
            assert!(report.diagnostics[0].message.contains("Failed to verify"));
        }
    }

    #[test]
    fn metadata_resolution_reports_broken_preferred_style_without_changing_legacy_fallback() {
        let reader = archive(vec![
            ("app.json", br#"{"pages":["home"]}"#.to_vec()),
            ("home.json", b"{}".to_vec()),
            ("home.wxml", b"<view class='card' />".to_vec()),
            ("home.wcss", vec![255]),
            ("home.wxss", b".card { width: 200px; }".to_vec()),
        ]);
        assert_eq!(
            reader.get_pages()[0].size.width,
            PageConstraint::default().width
        );
        let report = reader.resolve_metadata();
        assert!(report.pages.is_empty());
        assert_eq!(report.diagnostics[0].path, "home.wcss");
    }

    #[test]
    fn archive_limits_accept_exact_boundaries_and_preserve_unlimited_default() {
        let data = archive(vec![
            ("folder/", vec![]),
            ("folder/a", vec![1; 32]),
            (crypto::SIGNATURE_PATH, vec![2; 32]),
        ])
        .into_bytes();
        assert_eq!(
            AixReadLimits::default(),
            AixReadLimits {
                max_entries: None,
                max_entry_bytes: None,
                max_total_bytes: None,
            }
        );
        let reader = AixReader::new_with_limits(
            data.clone(),
            AixReadLimits {
                max_entries: Some(3),
                max_entry_bytes: Some(32),
                max_total_bytes: Some(64),
            },
        )
        .unwrap();
        assert_eq!(reader.list().len(), 3);
        assert!(reader.read_file("folder/").unwrap().is_empty());
        for _ in 0..3 {
            assert_eq!(reader.read_file("folder/a").unwrap(), vec![1; 32]);
            assert_eq!(
                reader.read_file(crypto::SIGNATURE_PATH).unwrap(),
                vec![2; 32]
            );
        }
        assert!(limited_archive_error(
            data.clone(),
            AixReadLimits {
                max_entries: Some(2),
                ..Default::default()
            }
        )
        .contains("entry count limit exceeded"));
        assert!(limited_archive_error(
            data.clone(),
            AixReadLimits {
                max_total_bytes: Some(63),
                ..Default::default()
            }
        )
        .contains("total byte limit exceeded"));
        assert_eq!(AixReader::new(data).unwrap().list().len(), 3);
    }

    #[test]
    fn zero_limits_allow_only_empty_archives_and_zero_byte_entries() {
        let empty = archive(vec![]).into_bytes();
        let limits = AixReadLimits {
            max_entries: Some(0),
            max_entry_bytes: Some(0),
            max_total_bytes: Some(0),
        };
        assert!(AixReader::new_with_limits(empty, limits).is_ok());
        let entry = archive(vec![("empty", vec![])]).into_bytes();
        assert!(limited_archive_error(entry.clone(), limits).contains("entry count limit"));
        let reader = AixReader::new_with_limits(
            entry,
            AixReadLimits {
                max_entries: Some(1),
                ..limits
            },
        )
        .unwrap();
        assert!(reader.read_file("empty").unwrap().is_empty());
        let nonempty = archive(vec![("a", vec![1])]).into_bytes();
        assert!(limited_archive_error(
            nonempty.clone(),
            AixReadLimits {
                max_entry_bytes: Some(0),
                ..Default::default()
            }
        )
        .contains("per-entry byte limit"));
        assert!(limited_archive_error(
            nonempty,
            AixReadLimits {
                max_total_bytes: Some(0),
                ..Default::default()
            }
        )
        .contains("total byte limit"));
    }

    #[test]
    fn rejects_oversized_metadata_before_extraction_for_stored_and_deflated_entries() {
        for method in [
            zip::CompressionMethod::Stored,
            zip::CompressionMethod::Deflated,
        ] {
            let data = entry_fixture(method);
            let limits = AixReadLimits {
                max_entries: Some(1),
                max_entry_bytes: Some(4096),
                max_total_bytes: Some(4096),
            };
            assert_eq!(
                AixReader::new(data.clone())
                    .unwrap()
                    .read_file("payload")
                    .unwrap()
                    .len(),
                4096
            );
            assert_eq!(
                AixReader::new_with_limits(data.clone(), limits)
                    .unwrap()
                    .read_file("payload")
                    .unwrap()
                    .len(),
                4096
            );
            assert!(limited_archive_error(
                data.clone(),
                AixReadLimits {
                    max_entry_bytes: Some(4095),
                    ..limits
                }
            )
            .contains("per-entry byte limit"));
            assert!(limited_archive_error(
                data,
                AixReadLimits {
                    max_total_bytes: Some(4095),
                    ..limits
                }
            )
            .contains("total byte limit"));
        }
    }

    #[test]
    fn forged_sizes_cannot_expand_or_copy_beyond_output_limit() {
        for method in [
            zip::CompressionMethod::Stored,
            zip::CompressionMethod::Deflated,
        ] {
            let mut data = entry_fixture(method);
            let offset = central_offset(&data);
            // Actual output is 4096 bytes; both the declaration and host budget are 64.
            data[offset + 24..offset + 28].copy_from_slice(&64u32.to_le_bytes());
            let reader = AixReader::new_with_limits(
                data,
                AixReadLimits {
                    max_entry_bytes: Some(64),
                    max_total_bytes: Some(64),
                    ..Default::default()
                },
            )
            .unwrap();
            let error = reader.read_file("payload").unwrap_err().to_string();
            assert!(
                error.contains(if method == zip::CompressionMethod::Stored {
                    "Extracted file size limit exceeded"
                } else {
                    "Failed to extract"
                }),
                "{error}"
            );
        }
    }

    #[test]
    fn limited_reads_retain_crc_and_size_verification() {
        for method in [
            zip::CompressionMethod::Stored,
            zip::CompressionMethod::Deflated,
        ] {
            for field in [16, 24] {
                let mut data = entry_fixture(method);
                let offset = central_offset(&data);
                // Change either the expected CRC or the expected uncompressed size.
                data[offset + field] ^= 1;
                let reader = AixReader::new_with_limits(
                    data,
                    AixReadLimits {
                        max_entry_bytes: Some(8192),
                        ..Default::default()
                    },
                )
                .unwrap();
                assert!(reader
                    .read_file("payload")
                    .unwrap_err()
                    .to_string()
                    .contains("Failed to verify"));
            }
        }
    }

    #[test]
    fn entry_budget_counts_actual_directory_entries_despite_forged_eocd_hint() {
        let mut data = archive(vec![("a", vec![]), ("b", vec![])]).into_bytes();
        let end = data.len() - 22;
        data[end + 8..end + 12].fill(0);
        assert!(limited_archive_error(
            data,
            AixReadLimits {
                max_entries: Some(1),
                ..Default::default()
            }
        )
        .contains("entry count limit"));
    }

    #[test]
    fn aggregate_zip64_size_overflow_is_rejected_without_extracting() {
        let mut data = archive(vec![("a", vec![]), ("b", vec![1])]).into_bytes();
        let offset = central_offset(&data);
        let name_len =
            u16::from_le_bytes(data[offset + 28..offset + 30].try_into().unwrap()) as usize;
        let extra_len = u16::from_le_bytes(data[offset + 30..offset + 32].try_into().unwrap());
        data[offset + 24..offset + 28].copy_from_slice(&u32::MAX.to_le_bytes());
        data[offset + 30..offset + 32].copy_from_slice(&(extra_len + 12).to_le_bytes());
        let mut extra = vec![1, 0, 8, 0]; // ZIP64 field with one u64 uncompressed size.
        extra.extend_from_slice(&u64::MAX.to_le_bytes());
        data.splice(offset + 46 + name_len..offset + 46 + name_len, extra);
        let end = data.len() - 22;
        let size = u32::from_le_bytes(data[end + 12..end + 16].try_into().unwrap());
        data[end + 12..end + 16].copy_from_slice(&(size + 12).to_le_bytes());
        assert!(limited_archive_error(data, AixReadLimits::default()).contains("size overflow"));
    }

    #[test]
    fn rejects_each_signed_manifest_invariant_even_with_a_valid_signature() {
        let key = crypto::PrivateKey::from_seed([7; 32]);
        let entries = vec![
            crypto::ManifestEntry {
                path: "VERSION".into(),
                size: 5,
                sha256: crypto::sha256(b"build"),
            },
            crypto::ManifestEntry {
                path: "app.json".into(),
                size: 12,
                sha256: crypto::sha256(br#"{"pages":[]}"#),
            },
        ];
        let manifest = crypto::PackageManifest {
            agent_id: None,
            format: "aix".into(),
            version: "build".into(),
            engine: "*".into(),
            algorithm: "ed25519".into(),
            digest: "sha256".into(),
            key_id: key.public_key().key_id(),
            package_id: crypto::calculate_package_id(&entries),
            entries,
        };
        let fixture = |manifest: &crypto::PackageManifest,
                       payload: Vec<(&str, Vec<u8>)>,
                       signature: Option<Vec<u8>>| {
            let bytes = serde_json::to_vec(manifest).unwrap();
            let signature = signature
                .unwrap_or_else(|| key.sign(b"package-manifest", &bytes).to_bytes().to_vec());
            let mut files = payload;
            files.push((crypto::MANIFEST_PATH, bytes));
            files.push((crypto::SIGNATURE_PATH, signature));
            archive(files)
        };
        let payload = || {
            vec![
                ("VERSION", b"build".to_vec()),
                ("app.json", br#"{"pages":[]}"#.to_vec()),
                ("folder/", vec![]),
            ]
        };
        assert_eq!(
            fixture(&manifest, payload(), None)
                .verify_signature(&key.public_key())
                .unwrap()
                .entry_count,
            2
        );
        // Identity metadata is covered by the original manifest signature.
        let original_signature =
            key.sign(b"package-manifest", &serde_json::to_vec(&manifest).unwrap());
        let mut altered_identity = manifest.clone();
        altered_identity.agent_id = Some("different-project".into());
        assert!(fixture(
            &altered_identity,
            payload(),
            Some(original_signature.to_bytes().to_vec())
        )
        .verify_signature(&key.public_key())
        .unwrap_err()
        .to_string()
        .contains("AIX signature verification failed"));
        enum Mutation {
            Format,
            Engine,
            Key,
            Order,
            Metadata,
            Size,
            Digest,
            Version,
            Id,
        }
        for (case, expected) in [
            (Mutation::Format, "Unsupported AIX signature manifest"),
            (Mutation::Engine, "Invalid engine range"),
            (Mutation::Key, "Manifest key ID does not match"),
            (Mutation::Order, "Invalid or unsorted manifest entry"),
            (Mutation::Metadata, "Invalid or unsorted manifest entry"),
            (Mutation::Size, "Package entry digest mismatch"),
            (Mutation::Digest, "Package entry digest mismatch"),
            (Mutation::Version, "Manifest version does not match"),
            (Mutation::Id, "Manifest package ID mismatch"),
        ] {
            let mut altered = manifest.clone();
            match case {
                Mutation::Format => altered.format = "other".into(),
                Mutation::Engine => altered.engine = "invalid".into(),
                Mutation::Key => altered.key_id = "wrong".into(),
                Mutation::Order => altered.entries.reverse(),
                Mutation::Metadata => altered.entries[0].path = crypto::MANIFEST_PATH.into(),
                Mutation::Size => altered.entries[0].size += 1,
                Mutation::Digest => altered.entries[0].sha256 = "wrong".into(),
                Mutation::Version => altered.version = "different".into(),
                Mutation::Id => altered.package_id = "wrong".into(),
            }
            let error = fixture(&altered, payload(), None)
                .verify_signature(&key.public_key())
                .unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }
        for (signature, expected) in [
            (vec![0], "Invalid Ed25519 signature length"),
            (vec![0; 64], "AIX signature verification failed"),
        ] {
            assert!(fixture(&manifest, payload(), Some(signature))
                .verify_signature(&key.public_key())
                .unwrap_err()
                .to_string()
                .contains(expected));
        }
        let mut extra = payload();
        extra.push(("unsigned", b"extra".to_vec()));
        assert!(fixture(&manifest, extra, None)
            .verify_signature(&key.public_key())
            .unwrap_err()
            .to_string()
            .contains("Unsigned package entry"));
        let mut without_version = manifest;
        without_version.entries.remove(0);
        assert!(fixture(
            &without_version,
            vec![("app.json", br#"{"pages":[]}"#.to_vec())],
            None
        )
        .verify_signature(&key.public_key())
        .unwrap_err()
        .to_string()
        .contains("VERSION entry not found"));
        assert!(archive(vec![(crypto::MANIFEST_PATH, b"{broken".to_vec())])
            .verify_signature(&key.public_key())
            .unwrap_err()
            .to_string()
            .contains("Invalid AIX manifest"));
    }

    #[test]
    fn page_metadata_defaults_survive_missing_schema_and_invalid_inputs() {
        for config in [b"{}".as_slice(), br#"{"schema":{}}"#] {
            for extension in ["home.ink", "home.json"] {
                let body = if extension.ends_with("ink") {
                    [
                        b"<script def>".as_slice(),
                        config,
                        b"</script><page><view /></page>",
                    ]
                    .concat()
                } else {
                    config.to_vec()
                };
                let reader = archive(vec![
                    ("app.json", br#"{"pages":["home"]}"#.to_vec()),
                    (extension, body),
                ]);
                assert_eq!(reader.get_pages()[0].data_schema, serde_json::json!({}));
                assert_eq!(reader.get_tools()[0].target, ToolTarget::Blank);
            }
        }
        let reader = archive(vec![
            ("app.json", b"invalid".to_vec()),
            ("VERSION", vec![0xff]),
        ]);
        assert!(reader.get_pages().is_empty());
        assert!(reader.get_title().is_none());
        assert!(reader.get_version().is_none());
        assert!(satisfy("invalid", "1.0.0").is_err());
        assert!(!page_has_parameters(&serde_json::Value::Null));
        assert!(page_has_parameters(&serde_json::json!(true)));
        for (bytes, text) in [
            (1, "1 bytes"),
            (1024, "1.00 KB"),
            (1 << 20, "1.00 MB"),
            (1 << 30, "1.00 GB"),
        ] {
            assert_eq!(format_size(bytes), text);
        }
        assert_eq!(reader.find_locale_file("  "), None);
        let reader = archive(vec![
            ("app.json", br#"{"pages":[],"widgets":[]}"#.to_vec()),
            ("app.en.json", b"invalid".to_vec()),
        ]);
        assert!(reader
            .get_widgets_for_locale("en")
            .unwrap_err()
            .to_string()
            .contains("Invalid locale configuration"));
    }

    fn create_test_aix() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();

            zip.start_file("app.json", options).unwrap();
            zip.write_all(
                br#"{
                "pages": ["pages/index/index"],
                "window": { "navigationBarTitleText": "Test App" }
            }"#,
            )
            .unwrap();

            zip.start_file("pages/index/index.json", options).unwrap();
            zip.write_all(
                br#"{
                "navigationBarTitleText": "Index Page",
                "schema": {
                    "data": {
                        "type": "object",
                        "properties": {
                            "test": { "type": "string" }
                        }
                    }
                }
            }"#,
            )
            .unwrap();

            zip.start_file("pages/index/index.wxml", options).unwrap();
            zip.write_all(br#"<view class="container"></view>"#)
                .unwrap();

            zip.start_file("pages/index/index.wxss", options).unwrap();
            zip.write_all(br#".container { width: 100px; height: 100px; }"#)
                .unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    fn create_widget_test_aix(include_entry: bool, placement: Option<&str>) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();

            zip.start_file("app.json", options).unwrap();
            let widget = match placement {
                Some(placement) => format!(
                    r#"{{"pages":[],"widgets":[{{"path":"widgets/clock/index","family":"1x1","placement":"{placement}"}}]}}"#
                ),
                None => r#"{"pages":[],"widgets":[{"path":"widgets/clock/index","family":"1x1"}]}"#
                    .to_string(),
            };
            zip.write_all(widget.as_bytes()).unwrap();

            if include_entry {
                zip.start_file("widgets/clock/index.ink", options).unwrap();
                zip.write_all(b"<widget></widget>").unwrap();
            }

            zip.finish().unwrap();
        }
        buf
    }

    fn create_localized_widget_test_aix() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();
            zip.start_file("app.json", options).unwrap();
            zip.write_all(br#"{"pages":[],"widgets":[{"path":"widgets/clock/index","family":"1x1","displayName":"\u65f6\u949f","description":"\u663e\u793a\u5f53\u524d\u65f6\u95f4\u3002"}]}"#).unwrap();
            zip.start_file("app.en-US.json", options).unwrap();
            zip.write_all(br#"{"locale":"en-US","widgets":{"widgets/clock/index":{"displayName":"Clock","description":"Shows the current time."}}}"#).unwrap();
            zip.start_file("app.zh-TW.json", options).unwrap();
            zip.write_all(br#"{"locale":"zh-TW","widgets":{"widgets/clock/index":{"displayName":"\u6642\u9418"}}}"#).unwrap();
            zip.start_file("widgets/clock/index.ink", options).unwrap();
            zip.write_all(b"<widget></widget>").unwrap();
            zip.finish().unwrap();
        }
        buf
    }

    fn create_engine_test_aix(app_json: &[u8], manifest_engine: Option<&str>) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();

            zip.start_file("app.json", options).unwrap();
            zip.write_all(app_json).unwrap();

            if let Some(engine) = manifest_engine {
                zip.start_file("VERSION", options).unwrap();
                zip.write_all(b"test-build").unwrap();

                let manifest = crypto::PackageManifest {
                    agent_id: None,
                    format: "aix".into(),
                    version: "test-build".into(),
                    engine: engine.into(),
                    algorithm: "ed25519".into(),
                    digest: "sha256".into(),
                    key_id: String::new(),
                    package_id: String::new(),
                    entries: Vec::new(),
                };
                zip.start_file(crypto::MANIFEST_PATH, options).unwrap();
                zip.write_all(&serde_json::to_vec(&manifest).unwrap())
                    .unwrap();
            }

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn reads_deflated_entries() {
        let mut data = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut data));
            let options =
                FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("app.json", options).unwrap();
            zip.write_all(br#"{"pages":[]}"#).unwrap();
            zip.finish().unwrap();
        }

        let reader = AixReader::new(data).unwrap();
        assert_eq!(reader.read_file("app.json").unwrap(), br#"{"pages":[]}"#);
    }

    #[test]
    fn returns_declared_widgets_when_entries_exist() {
        let reader = AixReader::new(create_widget_test_aix(true, None)).unwrap();

        assert_eq!(
            reader.get_widgets().unwrap(),
            vec![WidgetInfo {
                path: "widgets/clock/index".to_string(),
                family: "1x1".to_string(),
                placement: WidgetPlacement::Persistent,
                display_name: None,
                description: None,
            }]
        );
    }

    #[test]
    fn rejects_widget_with_missing_entry() {
        let reader = AixReader::new(create_widget_test_aix(false, None)).unwrap();

        assert_eq!(
            reader.get_widgets().unwrap_err().to_string(),
            "Widget entry not found: widgets/clock/index.ink"
        );
    }

    #[test]
    fn reads_overlay_widget_placement() {
        let reader = AixReader::new(create_widget_test_aix(true, Some("overlay"))).unwrap();

        assert_eq!(
            reader.get_widgets().unwrap()[0].placement,
            WidgetPlacement::Overlay
        );
    }

    #[test]
    fn resolves_widget_metadata_from_locale_overlay() {
        let reader = AixReader::new(create_localized_widget_test_aix()).unwrap();
        let widgets = reader.get_widgets_for_locale("en-US").unwrap();

        assert_eq!(widgets[0].display_name.as_deref(), Some("Clock"));
        assert_eq!(
            widgets[0].description.as_deref(),
            Some("Shows the current time.")
        );
    }

    #[test]
    fn falls_back_to_default_widget_metadata_for_unknown_locale() {
        let reader = AixReader::new(create_localized_widget_test_aix()).unwrap();
        let widgets = reader.get_widgets_for_locale("fr-FR").unwrap();

        assert_eq!(widgets[0].display_name.as_deref(), Some("\u{65f6}\u{949f}"));
        assert_eq!(
            widgets[0].description.as_deref(),
            Some("\u{663e}\u{793a}\u{5f53}\u{524d}\u{65f6}\u{95f4}\u{3002}")
        );
    }

    #[test]
    fn falls_back_per_field_when_locale_overlay_is_partial() {
        let reader = AixReader::new(create_localized_widget_test_aix()).unwrap();
        let widgets = reader.get_widgets_for_locale("zh-TW").unwrap();

        assert_eq!(widgets[0].display_name.as_deref(), Some("\u{6642}\u{9418}"));
        assert_eq!(
            widgets[0].description.as_deref(),
            Some("\u{663e}\u{793a}\u{5f53}\u{524d}\u{65f6}\u{95f4}\u{3002}")
        );
    }

    #[test]
    fn returns_empty_widgets_for_legacy_app_config() {
        let reader = AixReader::new(create_engine_test_aix(br#"{"pages":[]}"#, None)).unwrap();

        assert!(reader.get_widgets().unwrap().is_empty());
    }

    #[test]
    fn missing_manifest_returns_none() {
        let reader = AixReader::new(create_test_aix()).unwrap();
        assert!(reader.get_manifest().unwrap().is_none());
        assert!(reader.get_agent_id().unwrap().is_none());
    }

    #[test]
    fn agent_identity_is_optional_and_rejects_invalid_present_metadata() {
        let reader = AixReader::new(create_engine_test_aix(br#"{"pages":[]}"#, Some("*"))).unwrap();
        assert!(reader.get_agent_id().unwrap().is_none());
        let mut manifest = serde_json::to_value(reader.get_manifest().unwrap().unwrap()).unwrap();
        assert!(manifest.get("agent_id").is_none());
        for id in [
            serde_json::json!("stable-project_1"),
            serde_json::json!(""),
            serde_json::json!("!!!"),
            serde_json::json!(42),
        ] {
            manifest["agent_id"] = id.clone();
            let reader = archive(vec![(
                crypto::MANIFEST_PATH,
                serde_json::to_vec(&manifest).unwrap(),
            )]);
            if id == serde_json::json!("stable-project_1") {
                assert_eq!(
                    reader.get_agent_id().unwrap().as_deref(),
                    Some("stable-project_1")
                );
            } else {
                assert!(reader
                    .get_agent_id()
                    .unwrap_err()
                    .to_string()
                    .contains("Invalid AIX manifest"));
            }
        }
        let reader = archive(vec![(crypto::MANIFEST_PATH, b"{".to_vec())]);
        assert!(reader.get_agent_id().is_err());
    }

    #[test]
    fn corrupt_manifest_propagates_read_error() {
        let manifest = br#"{"format":"aix"}"#;
        let mut data = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut data));
            let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            zip.start_file(crypto::MANIFEST_PATH, options).unwrap();
            zip.write_all(manifest).unwrap();
            zip.finish().unwrap();
        }
        let offset = data
            .windows(manifest.len())
            .position(|window| window == manifest)
            .unwrap();
        data[offset] ^= 1;

        let reader = AixReader::new(data).unwrap();
        let error = reader.get_manifest().unwrap_err();
        assert!(error.to_string().contains("Failed to verify"));
    }

    #[test]
    fn test_aix_reader_metadata() {
        let data = create_test_aix();
        let reader = AixReader::new(data).unwrap();

        assert_eq!(reader.get_title(), Some("Test App".to_string()));

        let pages = reader.get_pages();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].name, "pages/index/index");
        assert_eq!(pages[0].title, Some("Index Page".to_string()));
        assert_eq!(pages[0].data_schema["type"], "object");
        assert_eq!(pages[0].size.width, 100.0);
        assert_eq!(pages[0].size.height, 100.0);

        let tools = reader.get_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "pages/index/index");
        assert_eq!(tools[0].target, ToolTarget::Current);
        assert_eq!(
            tools[0].function.description,
            Some("Index Page".to_string())
        );
        assert_eq!(tools[0].function.parameters["type"], "object");
        assert_eq!(tools[0].layout.width, 100.0);
        assert_eq!(tools[0].layout.height, 100.0);

        let tool_json = serde_json::to_value(&tools[0]).unwrap();
        assert_eq!(tool_json["target"], "_current");
        assert_eq!(tool_json["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn exposes_original_archive_bytes() {
        let data = create_test_aix();
        let reader = AixReader::new(data.clone()).unwrap();

        assert_eq!(reader.as_bytes(), data.as_slice());
        assert_eq!(reader.into_bytes(), data);
    }

    #[test]
    fn satisfy_checks_runtime_against_engine_range() {
        assert!(satisfy("^0.14.0", "0.14.9").unwrap());
        assert!(!satisfy("^0.14.0", "0.15.0").unwrap());
    }

    #[test]
    fn supports_engine_reads_manifest_engine() {
        let reader = AixReader::new(create_engine_test_aix(
            br#"{"pages":[],"engine":"^0.14.0"}"#,
            Some("^0.14.0"),
        ))
        .unwrap();

        assert_eq!(reader.get_engine().as_deref(), Some("^0.14.0"));
        assert!(reader.supports_engine("0.14.2").unwrap());
        assert!(!reader.supports_engine("0.15.0").unwrap());
    }

    #[test]
    fn supports_engine_ignores_app_json_engine_without_manifest() {
        let reader = AixReader::new(create_engine_test_aix(
            br#"{"pages":[],"engine":"^0.14.0"}"#,
            None,
        ))
        .unwrap();

        assert!(reader.get_engine().is_none());
        assert_eq!(
            reader.supports_engine("0.14.2").unwrap_err().to_string(),
            "AIX manifest not found"
        );
    }

    #[test]
    fn supports_engine_returns_none_without_manifest() {
        let reader = AixReader::new(create_test_aix()).unwrap();

        assert!(reader.get_engine().is_none());
    }

    fn create_test_sfc_aix() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();

            zip.start_file("app.json", options).unwrap();
            zip.write_all(
                br#"{
                "pages": ["pages/sfc_page/index"],
                "window": { "navigationBarTitleText": "Test SFC App" }
            }"#,
            )
            .unwrap();

            zip.start_file("pages/sfc_page/index.ink", options).unwrap();
            zip.write_all(
                br#"
<script def>
{
    "navigationBarTitleText": "SFC Page",
    "schema": {
        "data": {
            "type": "string"
        }
    }
}
</script>
<page>
    <view class="container"></view>
</page>
<style>
.container { width: 120px; height: 120px; }
</style>
"#,
            )
            .unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_aix_reader_sfc() {
        let data = create_test_sfc_aix();
        let reader = AixReader::new(data).unwrap();

        assert_eq!(reader.get_title(), Some("Test SFC App".to_string()));

        let pages = reader.get_pages();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].name, "pages/sfc_page/index");
        assert_eq!(pages[0].title, Some("SFC Page".to_string()));
        assert_eq!(pages[0].data_schema["type"], "string");
        assert_eq!(pages[0].size.width, 120.0);
        assert_eq!(pages[0].size.height, 120.0);
    }

    fn create_test_aix_wcss() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();

            zip.start_file("app.json", options).unwrap();
            zip.write_all(
                br#"{
                "pages": ["pages/index/index"],
                "window": { "navigationBarTitleText": "WCSS Test App" }
            }"#,
            )
            .unwrap();

            zip.start_file("pages/index/index.json", options).unwrap();
            zip.write_all(
                br#"{
                "navigationBarTitleText": "WCSS Page",
                "schema": { "data": { "type": "object" } }
            }"#,
            )
            .unwrap();

            zip.start_file("pages/index/index.wxml", options).unwrap();
            zip.write_all(br#"<view class="wrapper"></view>"#).unwrap();

            // Use .wcss extension (preferred over .wxss)
            zip.start_file("pages/index/index.wcss", options).unwrap();
            zip.write_all(br#".wrapper { width: 240px; height: 80px; }"#)
                .unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_aix_reader_wcss_extension() {
        let data = create_test_aix_wcss();
        let reader = AixReader::new(data).unwrap();

        let pages = reader.get_pages();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].size.width, 240.0);
        assert_eq!(pages[0].size.height, 80.0);
    }

    fn create_test_aix_no_dimensions() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();

            zip.start_file("app.json", options).unwrap();
            zip.write_all(
                br#"{
                "pages": ["pages/index/index"],
                "window": { "navigationBarTitleText": "No Dim App" }
            }"#,
            )
            .unwrap();

            zip.start_file("pages/index/index.json", options).unwrap();
            zip.write_all(
                br#"{
                "navigationBarTitleText": "No Dim Page",
                "schema": { "data": { "type": "object" } }
            }"#,
            )
            .unwrap();

            zip.start_file("pages/index/index.wxml", options).unwrap();
            zip.write_all(br#"<view class="container"></view>"#)
                .unwrap();

            // CSS has no width/height properties
            zip.start_file("pages/index/index.wxss", options).unwrap();
            zip.write_all(br#".container { color: blue; background: white; }"#)
                .unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_aix_reader_default_size_when_no_dimensions() {
        let data = create_test_aix_no_dimensions();
        let reader = AixReader::new(data).unwrap();

        let pages = reader.get_pages();
        assert_eq!(pages.len(), 1);
        // Should fall back to default dimensions
        assert_eq!(pages[0].size.width, PageConstraint::default().width);
        assert_eq!(pages[0].size.height, PageConstraint::default().height);
    }

    fn create_test_aix_multiple_pages() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();

            zip.start_file("app.json", options).unwrap();
            zip.write_all(
                br#"{
                "pages": ["pages/home/index", "pages/detail/index"],
                "window": { "navigationBarTitleText": "Multi Page App" }
            }"#,
            )
            .unwrap();

            // First page: traditional files
            zip.start_file("pages/home/index.json", options).unwrap();
            zip.write_all(
                br#"{
                "navigationBarTitleText": "Home",
                "description": "Home page",
                "schema": { "data": { "type": "object" } }
            }"#,
            )
            .unwrap();
            zip.start_file("pages/home/index.wxml", options).unwrap();
            zip.write_all(br#"<view class="home"></view>"#).unwrap();
            zip.start_file("pages/home/index.wxss", options).unwrap();
            zip.write_all(br#".home { width: 480px; height: 168px; }"#)
                .unwrap();

            // Second page: SFC format
            zip.start_file("pages/detail/index.ink", options).unwrap();
            zip.write_all(
                br#"
<script def>
{
    "navigationBarTitleText": "Detail",
    "description": "Detail page",
    "schema": { "data": { "type": "string" } }
}
</script>
<page>
<view class="detail"></view>
</page>
<style>
.detail { width: 320px; height: 240px; }
</style>
"#,
            )
            .unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_aix_reader_multiple_pages() {
        let data = create_test_aix_multiple_pages();
        let reader = AixReader::new(data).unwrap();

        let pages = reader.get_pages();
        assert_eq!(pages.len(), 2);

        // First page: traditional files
        assert_eq!(pages[0].name, "pages/home/index");
        assert_eq!(pages[0].title, Some("Home".to_string()));
        assert_eq!(pages[0].size.width, 480.0);
        assert_eq!(pages[0].size.height, 168.0);

        // Second page: SFC format
        assert_eq!(pages[1].name, "pages/detail/index");
        assert_eq!(pages[1].title, Some("Detail".to_string()));
        assert_eq!(pages[1].size.width, 320.0);
        assert_eq!(pages[1].size.height, 240.0);

        let tools = reader.get_tools();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[1].target, ToolTarget::Current);
        assert_eq!(tools[0].target, ToolTarget::Current);
        assert_eq!(tools[0].function.name, "pages/home/index");
        assert_eq!(tools[1].function.name, "pages/detail/index");
        assert_eq!(tools[0].function.parameters["type"], "object");
        assert_eq!(tools[1].function.parameters["type"], "string");
        assert_eq!(tools[0].layout.width, 480.0);
        assert_eq!(tools[0].layout.height, 168.0);
        assert_eq!(tools[1].layout.width, 320.0);
        assert_eq!(tools[1].layout.height, 240.0);
    }

    fn create_test_aix_empty_schema_first_page() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();

            zip.start_file("app.json", options).unwrap();
            zip.write_all(
                br#"{
                "pages": ["pages/home/index", "pages/detail/index"],
                "window": { "navigationBarTitleText": "Empty Schema App" }
            }"#,
            )
            .unwrap();

            zip.start_file("pages/home/index.json", options).unwrap();
            zip.write_all(
                br#"{
                "navigationBarTitleText": "Home",
                "description": "Home page",
                "schema": { "data": {} }
            }"#,
            )
            .unwrap();
            zip.start_file("pages/home/index.wxml", options).unwrap();
            zip.write_all(br#"<view class="home"></view>"#).unwrap();
            zip.start_file("pages/home/index.wxss", options).unwrap();
            zip.write_all(br#".home { width: 200px; height: 100px; }"#)
                .unwrap();

            zip.start_file("pages/detail/index.json", options).unwrap();
            zip.write_all(
                br#"{
                "navigationBarTitleText": "Detail",
                "description": "Detail page",
                "schema": { "data": { "type": "object" } }
            }"#,
            )
            .unwrap();
            zip.start_file("pages/detail/index.wxml", options).unwrap();
            zip.write_all(br#"<view class="detail"></view>"#).unwrap();
            zip.start_file("pages/detail/index.wxss", options).unwrap();
            zip.write_all(br#".detail { width: 300px; height: 150px; }"#)
                .unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_aix_reader_first_page_without_parameters_only_blank() {
        let data = create_test_aix_empty_schema_first_page();
        let reader = AixReader::new(data).unwrap();

        let tools = reader.get_tools();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].target, ToolTarget::Blank);
        assert_eq!(tools[0].function.name, "pages/home/index");
        assert_eq!(tools[0].function.parameters, serde_json::json!({}));
        assert_eq!(tools[1].target, ToolTarget::Current);
        assert_eq!(tools[1].function.name, "pages/detail/index");
    }

    fn create_test_aix_empty_properties_schema_first_page() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();

            zip.start_file("app.json", options).unwrap();
            zip.write_all(
                br#"{
                "pages": ["pages/home/index", "pages/detail/index"],
                "window": { "navigationBarTitleText": "Empty Properties App" }
            }"#,
            )
            .unwrap();

            zip.start_file("pages/home/index.json", options).unwrap();
            zip.write_all(
                br#"{
                "navigationBarTitleText": "Home",
                "description": "Home page",
                "schema": {
                    "data": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                }
            }"#,
            )
            .unwrap();
            zip.start_file("pages/home/index.wxml", options).unwrap();
            zip.write_all(br#"<view class="home"></view>"#).unwrap();
            zip.start_file("pages/home/index.wxss", options).unwrap();
            zip.write_all(br#".home { width: 200px; height: 100px; }"#)
                .unwrap();

            zip.start_file("pages/detail/index.json", options).unwrap();
            zip.write_all(
                br#"{
                "navigationBarTitleText": "Detail",
                "description": "Detail page",
                "schema": { "data": { "type": "object", "properties": { "id": { "type": "string" } } } }
            }"#,
            )
            .unwrap();
            zip.start_file("pages/detail/index.wxml", options).unwrap();
            zip.write_all(br#"<view class="detail"></view>"#).unwrap();
            zip.start_file("pages/detail/index.wxss", options).unwrap();
            zip.write_all(br#".detail { width: 300px; height: 150px; }"#)
                .unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_aix_reader_first_page_empty_properties_schema_is_blank() {
        let data = create_test_aix_empty_properties_schema_first_page();
        let reader = AixReader::new(data).unwrap();

        let tools = reader.get_tools();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].target, ToolTarget::Blank);
        assert_eq!(tools[0].function.name, "pages/home/index");
        assert_eq!(tools[0].function.parameters, serde_json::json!({}));
        assert_eq!(tools[1].target, ToolTarget::Current);
        assert_eq!(tools[1].function.name, "pages/detail/index");
    }

    fn create_test_sfc_aix_inline_style() -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = FileOptions::default();

            zip.start_file("app.json", options).unwrap();
            zip.write_all(
                br#"{
                "pages": ["pages/inline/index"],
                "window": { "navigationBarTitleText": "Inline Style App" }
            }"#,
            )
            .unwrap();

            zip.start_file("pages/inline/index.ink", options).unwrap();
            zip.write_all(
                br#"
<script def>
{
    "navigationBarTitleText": "Inline Style Page",
    "schema": { "data": { "type": "object" } }
}
</script>
<page>
<view style="width: 400px; height: 180px;"></view>
</page>
<style>
</style>
"#,
            )
            .unwrap();

            zip.finish().unwrap();
        }
        buf
    }

    #[test]
    fn test_aix_reader_sfc_inline_style() {
        let data = create_test_sfc_aix_inline_style();
        let reader = AixReader::new(data).unwrap();

        let pages = reader.get_pages();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].title, Some("Inline Style Page".to_string()));
        assert_eq!(pages[0].size.width, 400.0);
        assert_eq!(pages[0].size.height, 180.0);
    }
}
