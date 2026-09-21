#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{format, string::String, string::ToString, vec::Vec};
use anyhow::{anyhow, Result};
#[cfg(not(feature = "std"))]
use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = warn)]
    fn console_warn(s: &str);
}

fn aix_warn(msg: &str) {
    #[cfg(feature = "wasm")]
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
}

/// Describes how the host manages a Widget's placement.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetPlacement {
    Persistent,
    Overlay,
}

impl Default for WidgetPlacement {
    fn default() -> Self {
        Self::Persistent
    }
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
struct AppConfig {
    pub pages: Vec<String>,
    #[serde(default)]
    pub widgets: Vec<WidgetInfo>,
    pub window: Option<WindowConfig>,
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
        let mut entries = Vec::new();
        let mut index = HashMap::new();
        let archive = ZipArchive::from_slice(data.as_slice())
            .map_err(|error| anyhow!("Failed to read zip: {:?}", error))?;
        let mut zip_entries = archive.entries();
        while let Some(file) = zip_entries
            .next_entry()
            .map_err(|error| anyhow!("Failed to read zip entry: {:?}", error))?
        {
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
        let limit = usize::try_from(claimed.uncompressed_size)
            .map_err(|_| anyhow!("File too large: {}", name))?;
        let output = match locator.method {
            CompressionMethod::STORE => local.data().to_vec(),
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

    fn read_app_config(&self) -> Result<AppConfig> {
        let app_json = self.read_file("app.json")?;
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
        let config = self.read_app_config().ok()?;
        config.window.and_then(|w| w.navigation_bar_title_text)
    }

    /// Resolves every page declared in `app.json` into page metadata.
    ///
    /// This method loads the page list, extracts titles and schema information
    /// from either `.ink` single-file components or traditional page JSON, and
    /// derives layout constraints from templates and styles.
    ///
    /// # Returns
    ///
    /// Returns one [`PageInfo`] per page declared in `app.json`. If the package
    /// is missing or has an invalid `app.json`, an empty vector is returned.
    pub fn get_pages(&self) -> Vec<PageInfo> {
        let mut pages = Vec::new();
        if let Ok(config) = self.read_app_config() {
            for path in config.pages {
                let mut title = None;
                let mut description = None;
                let mut data_schema = serde_json::json!({});
                let mut size = PageConstraint::default();

                // Check if it's an SFC (.ink file) first
                let ink_path = format!("{}.ink", path);
                if let Ok(ink_content) = self
                    .read_file(&ink_path)
                    .and_then(|b| String::from_utf8(b).map_err(|e| anyhow::anyhow!(e)))
                {
                    // Extract config from <script def> and template/style for analyzer
                    if let Ok(nodes) = xml::parse_sfc(&ink_content) {
                        for node in &nodes {
                            if let xml::Node::Element {
                                name,
                                attributes,
                                children,
                            } = node
                            {
                                if name == "script" && attributes.contains_key("def") {
                                    if let Some(xml::Node::Text(text)) = children.first() {
                                        if let Ok(page_config) =
                                            serde_json::from_str::<PageConfig>(text)
                                        {
                                            title = page_config.navigation_bar_title_text;
                                            description = page_config.description;
                                            match page_config.schema {
                                                Some(schema) => match schema.data {
                                                    Some(data) => {
                                                        data_schema = data;
                                                    }
                                                    None => {
                                                        aix_warn(&format!(
                                                            "Missing 'data' in schema for page: {}",
                                                            path
                                                        ));
                                                    }
                                                },
                                                None => {
                                                    aix_warn(&format!(
                                                        "Missing 'schema' for page: {}",
                                                        path
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    size = PageAnalyzer::analyze_sfc(&ink_content);
                } else {
                    // Fallback to traditional files
                    let json_path = format!("{}.json", path);
                    if let Ok(page_json) = self.read_file(&json_path) {
                        if let Ok(page_config) = serde_json::from_slice::<PageConfig>(&page_json) {
                            title = page_config.navigation_bar_title_text;
                            description = page_config.description;
                            match page_config.schema {
                                Some(schema) => match schema.data {
                                    Some(data) => {
                                        data_schema = data;
                                    }
                                    None => {
                                        aix_warn(&format!(
                                            "Missing 'data' in schema for page: {}",
                                            path
                                        ));
                                    }
                                },
                                None => {
                                    aix_warn(&format!("Missing 'schema' for page: {}", path));
                                }
                            }
                        }
                    }

                    // Analyze page size
                    let wxml = self
                        .read_file(&format!("{}.wxml", path))
                        .ok()
                        .and_then(|b| String::from_utf8(b).ok());
                    let wcss = self
                        .read_file(&format!("{}.wcss", path))
                        .or_else(|_| self.read_file(&format!("{}.wxss", path)))
                        .ok()
                        .and_then(|b| String::from_utf8(b).ok());

                    if wxml.is_some() {
                        size = PageAnalyzer::analyze(wxml.as_deref(), wcss.as_deref());
                    }
                }

                pages.push(PageInfo {
                    name: path,
                    title,
                    description,
                    data_schema,
                    size,
                });
            }
        }
        pages
    }

    /// Returns the widgets declared in `app.json` after validating their entry files.
    ///
    /// Every widget path must resolve to an `.ink` single-file entry in the package.
    /// An absent `widgets` property is treated as an empty list.
    pub fn get_widgets(&self) -> Result<Vec<WidgetInfo>> {
        let config = self.read_app_config()?;
        for widget in &config.widgets {
            let entry_path = format!("{}.ink", widget.path);
            if !self.index.contains_key(entry_path.as_str()) {
                return Err(anyhow!("Widget entry not found: {}", entry_path));
            }
        }
        Ok(config.widgets)
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
        let pages = self.get_pages();
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
    fn returns_empty_widgets_for_legacy_app_config() {
        let reader = AixReader::new(create_engine_test_aix(br#"{"pages":[]}"#, None)).unwrap();

        assert!(reader.get_widgets().unwrap().is_empty());
    }

    #[test]
    fn missing_manifest_returns_none() {
        let reader = AixReader::new(create_test_aix()).unwrap();
        assert!(reader.get_manifest().unwrap().is_none());
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
