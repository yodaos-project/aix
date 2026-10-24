use aix::{AixReadLimits, AixReader};
use aix_pack::{collector::CollectOptions, InputFile, OptimizeOptions, PackOptions};
use anyhow::Result;
use js_sys::Function;
use wasm_bindgen::prelude::*;

fn to_value<T: serde::Serialize + ?Sized>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true))
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

// Metadata mirrors the native JSON models, including Option::None and nested
// serde_json::Value::Null. Other tooling exports keep their existing serializer.
fn to_metadata_value<T: serde::Serialize + ?Sized>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

#[wasm_bindgen]
pub struct AixReaderWasm {
    inner: AixReader,
}

#[wasm_bindgen]
pub struct AixPackResultWasm {
    data: Vec<u8>,
    report: aix_pack::OptimizeReport,
    warnings: Vec<String>,
}

#[wasm_bindgen]
pub struct AixSourcePackBuilderWasm {
    files: Vec<InputFile>,
}

impl Default for AixSourcePackBuilderWasm {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl AixPackResultWasm {
    #[wasm_bindgen(getter)]
    pub fn data(&self) -> js_sys::Uint8Array {
        js_sys::Uint8Array::from(self.data.as_slice())
    }

    #[wasm_bindgen(getter)]
    pub fn report(&self) -> Result<JsValue, JsValue> {
        to_value(&self.report)
    }

    #[wasm_bindgen(getter)]
    pub fn warnings(&self) -> Result<JsValue, JsValue> {
        to_value(&self.warnings)
    }
}

#[wasm_bindgen]
impl AixSourcePackBuilderWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> AixSourcePackBuilderWasm {
        AixSourcePackBuilderWasm { files: Vec::new() }
    }

    pub fn add_file(&mut self, path: String, data: Vec<u8>) {
        self.files.push(InputFile { path, data });
    }

    pub fn pack_from_source_with_progress(
        &mut self,
        build_id: String,
        engine: Option<String>,
        optimize_options: JsValue,
        progress: Function,
    ) -> Result<AixPackResultWasm, JsValue> {
        let optimize = parse_optimize_options(optimize_options)?;
        let files = std::mem::take(&mut self.files);
        let output = aix_pack::collector::pack_source_files_with_progress(
            files,
            &CollectOptions::default(),
            PackOptions {
                build_id,
                engine,
                optimize,
                signing_key: None,
            },
            |event| {
                emit_progress_event(&progress, &event)
                    .map_err(|error| anyhow::anyhow!("{:?}", error))
            },
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(AixPackResultWasm {
            data: output.data,
            report: output.report,
            warnings: output.warnings,
        })
    }
}

#[wasm_bindgen]
pub fn pack_aix(
    files: JsValue,
    build_id: String,
    engine: Option<String>,
    optimize_options: JsValue,
) -> Result<AixPackResultWasm, JsValue> {
    let files: Vec<InputFile> = serde_wasm_bindgen::from_value(files)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let optimize = parse_optimize_options(optimize_options)?;
    let output = pack_output(files, build_id, engine, optimize)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(AixPackResultWasm {
        data: output.data,
        report: output.report,
        warnings: output.warnings,
    })
}

#[wasm_bindgen]
pub fn pack_aix_from_source(
    files: JsValue,
    build_id: String,
    engine: Option<String>,
    optimize_options: JsValue,
) -> Result<AixPackResultWasm, JsValue> {
    let files: Vec<InputFile> = serde_wasm_bindgen::from_value(files)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let optimize = parse_optimize_options(optimize_options)?;
    let output = pack_output_from_source(files, build_id, engine, optimize)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(AixPackResultWasm {
        data: output.data,
        report: output.report,
        warnings: output.warnings,
    })
}

/// Collect a pre-optimization source snapshot using the packer's shared rules.
#[wasm_bindgen]
pub fn collect_aix_source(files: JsValue) -> Result<JsValue, JsValue> {
    let files: Vec<InputFile> = serde_wasm_bindgen::from_value(files)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let snapshot = aix_pack::collector::collect_source_snapshot(files, &CollectOptions::default())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    to_value(&snapshot)
}

/// Check source files using the same collection rules as the pack command.
#[wasm_bindgen]
pub fn check_aix_from_source(files: JsValue) -> Result<JsValue, JsValue> {
    let files: Vec<InputFile> = serde_wasm_bindgen::from_value(files)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let report = aix_pack::check::check_source(files)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    to_value(&report)
}

/// Check source files and return diagnostics with executed check coverage.
#[wasm_bindgen]
pub fn prepare_aix_check(files: JsValue) -> Result<JsValue, JsValue> {
    let files: Vec<InputFile> = serde_wasm_bindgen::from_value(files)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let prepared = aix_pack::check::prepare_check_source(files)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    to_value(&prepared)
}

#[wasm_bindgen]
pub fn pack_aix_from_source_with_progress(
    files: JsValue,
    build_id: String,
    engine: Option<String>,
    optimize_options: JsValue,
    progress: Function,
) -> Result<AixPackResultWasm, JsValue> {
    emit_progress_event(
        &progress,
        &aix_pack::PackProgressEvent::TransferringFilesToWasm,
    )?;
    let files: Vec<InputFile> = serde_wasm_bindgen::from_value(files)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let optimize = parse_optimize_options(optimize_options)?;
    let output = aix_pack::collector::pack_source_files_with_progress(
        files,
        &CollectOptions::default(),
        PackOptions {
            build_id,
            engine,
            optimize,
            signing_key: None,
        },
        |event| {
            emit_progress_event(&progress, &event).map_err(|error| anyhow::anyhow!("{:?}", error))
        },
    )
    .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(AixPackResultWasm {
        data: output.data,
        report: output.report,
        warnings: output.warnings,
    })
}

fn emit_progress_event(
    progress: &Function,
    event: &aix_pack::PackProgressEvent,
) -> std::result::Result<(), JsValue> {
    let value = to_value(event)?;
    progress
        .call1(&JsValue::NULL, &value)
        .map(|_| ())
        .map_err(|error| JsValue::from_str(&format!("{:?}", error)))
}

#[wasm_bindgen]
pub fn optimize_aix(data: Vec<u8>, options: JsValue) -> Result<AixPackResultWasm, JsValue> {
    let options: OptimizeOptions = if options.is_null() || options.is_undefined() {
        OptimizeOptions::default()
    } else {
        serde_wasm_bindgen::from_value(options)
            .map_err(|error| JsValue::from_str(&error.to_string()))?
    };
    let output = aix_pack::optimize_package(&data, &options)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(AixPackResultWasm {
        data: output.data,
        report: output.report,
        warnings: output.warnings,
    })
}

fn parse_optimize_options(optimize_options: JsValue) -> Result<Option<OptimizeOptions>, JsValue> {
    if optimize_options.is_null() || optimize_options.is_undefined() {
        Ok(None)
    } else {
        Ok(Some(
            serde_wasm_bindgen::from_value(optimize_options)
                .map_err(|error| JsValue::from_str(&error.to_string()))?,
        ))
    }
}

fn pack_output(
    files: Vec<InputFile>,
    build_id: String,
    engine: Option<String>,
    optimize: Option<OptimizeOptions>,
) -> Result<aix_pack::PackOutput> {
    aix_pack::pack(
        files,
        PackOptions {
            build_id,
            engine,
            optimize,
            signing_key: None,
        },
    )
}

fn pack_output_from_source(
    files: Vec<InputFile>,
    build_id: String,
    engine: Option<String>,
    optimize: Option<OptimizeOptions>,
) -> Result<aix_pack::PackOutput> {
    aix_pack::collector::pack_source_files(
        files,
        &CollectOptions::default(),
        PackOptions {
            build_id,
            engine,
            optimize,
            signing_key: None,
        },
    )
}

#[wasm_bindgen]
impl AixReaderWasm {
    /// Compatibility constructor without caller resource budgets.
    #[wasm_bindgen(constructor)]
    pub fn new(data: Vec<u8>) -> Result<AixReaderWasm, JsValue> {
        let inner = AixReader::new(data).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(AixReaderWasm { inner })
    }

    /// Create a reader with inclusive caller budgets. Omitted fields are unlimited.
    pub fn with_limits(data: Vec<u8>, limits: JsValue) -> Result<AixReaderWasm, JsValue> {
        if !limits.is_object() || limits.is_null() || js_sys::Array::is_array(&limits) {
            return Err(JsValue::from_str("Invalid read limits: expected an object"));
        }
        // serde-wasm-bindgen only visits known struct fields, so serde's
        // deny_unknown_fields cannot catch a misspelled budget on its own.
        for key in js_sys::Object::keys(limits.unchecked_ref::<js_sys::Object>()).iter() {
            if !matches!(
                key.as_string().as_deref(),
                Some("maxEntries" | "maxEntryBytes" | "maxTotalBytes")
            ) {
                return Err(JsValue::from_str(&format!(
                    "Invalid read limits: unknown field {:?}",
                    key.as_string()
                )));
            }
        }
        let limits: AixReadLimits = serde_wasm_bindgen::from_value(limits)
            .map_err(|error| JsValue::from_str(&format!("Invalid read limits: {}", error)))?;
        let inner = AixReader::new_with_limits(data, limits)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(AixReaderWasm { inner })
    }

    pub fn list(&self) -> Result<JsValue, JsValue> {
        to_value(&self.inner.list())
    }

    pub fn read_file(&self, name: &str) -> Result<Vec<u8>, JsValue> {
        self.inner
            .read_file(name)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn get_version(&self) -> Option<String> {
        self.inner.get_version()
    }

    pub fn supports_engine(&self, current_version: &str) -> Result<bool, JsValue> {
        self.inner
            .supports_engine(current_version)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn get_title(&self) -> Option<String> {
        self.inner.get_title()
    }

    pub fn get_pages(&self) -> Result<JsValue, JsValue> {
        to_metadata_value(&self.inner.get_pages())
    }

    /// Resolve readable pages/tools and expose per-resource failures as diagnostics.
    pub fn resolve_metadata(&self) -> Result<JsValue, JsValue> {
        to_metadata_value(&self.inner.resolve_metadata())
    }

    pub fn get_widgets(&self, locale: Option<String>) -> Result<JsValue, JsValue> {
        let widgets = match locale.as_deref() {
            Some(locale) => self.inner.get_widgets_for_locale(locale),
            None => self.inner.get_widgets(),
        }
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        to_metadata_value(&widgets)
    }

    pub fn get_tools(&self) -> Result<JsValue, JsValue> {
        to_metadata_value(&self.inner.get_tools())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    // The SDK tests compile and compare this native output instead of copying models.
    #[test]
    fn metadata_contract_fixture() -> Result<()> {
        use aix::{ToolTarget, WidgetPlacement};

        let files = vec![
            InputFile::new("app.json", br#"{"pages":["empty","rich","boolean","array","missing"],"widgets":[{"path":"plain","family":"1x1"},{"path":"named","family":"1x2","placement":"overlay","displayName":"Clock","description":"Widget description"},{"path":"empty-labels","family":"1x1","displayName":"","description":""}]}"#),
            InputFile::new("empty.ink", b"<page><view /></page>"),
            InputFile::new("rich.ink", br#"<script def>{"navigationBarTitleText":"Page title","description":"Page description","schema":{"data":{"type":"object","properties":{"value":{"type":["string","null"],"default":null,"enum":[null,"text",true,42,1.5,[false],{"nested":null}]}},"required":["value"]}}}</script><page><view style="width:320px;height:96px" /></page>"#),
            InputFile::new("boolean.ink", br#"<script def>{"navigationBarTitleText":"","description":"","schema":{"data":false}}</script><page><view /></page>"#),
            InputFile::new("array.ink", br#"<script def>{"schema":{"data":[null,true,7,"text",{"nested":null}]}}</script><page><view /></page>"#),
            InputFile::new("plain.ink", b"<widget><view /></widget>"),
            InputFile::new("named.ink", b"<widget><view /></widget>"),
            InputFile::new("empty-labels.ink", b"<widget><view /></widget>"),
        ];
        let packed = aix_pack::pack(files.clone(), PackOptions::new("metadata-contract"))?;
        let reader = AixReader::new(packed.data)?;
        println!(
            "AIX_METADATA_CONTRACT={}",
            serde_json::json!({
                "files": files,
                "expected": {
                    "pages": reader.get_pages(),
                    "tools": reader.get_tools(),
                    "widgets": reader.get_widgets()?,
                    "metadata": reader.resolve_metadata(),
                },
                "targets": [ToolTarget::Current, ToolTarget::Blank],
                "placements": [WidgetPlacement::Persistent, WidgetPlacement::Overlay],
            })
        );
        Ok(())
    }

    #[test]
    fn direct_wasm_pack_defaults_to_script_minification() {
        let output = pack_output(
            vec![
                InputFile {
                    path: "app.json".into(),
                    data: br#"{"pages":[]}"#.to_vec(),
                },
                InputFile {
                    path: "scripts/app.js".into(),
                    data: b"function demo(value) { return value + 1; }\n".to_vec(),
                },
            ],
            "test-build".into(),
            Some("*".into()),
            None,
        )
        .unwrap();

        let reader = AixReader::new(output.data).unwrap();
        let js_output = String::from_utf8(reader.read_file("scripts/app.js").unwrap()).unwrap();
        assert!(js_output.starts_with("function demo("));
        assert!(js_output.contains("return "));
        assert!(!js_output.contains("value"));

        let file_report = output
            .report
            .files
            .iter()
            .find(|file| file.path == "scripts/app.js")
            .unwrap();
        assert_eq!(file_report.status, aix_pack::OptimizeStatus::Optimized);
    }

    #[test]
    fn source_wasm_pack_defaults_to_script_minification() {
        let output = pack_output_from_source(
            vec![
                InputFile {
                    path: "app.json".into(),
                    data: br#"{"pages":[]}"#.to_vec(),
                },
                InputFile {
                    path: "scripts\\types.ts".into(),
                    data: b"export const total: number = 1 + 2;\n".to_vec(),
                },
            ],
            "test-build".into(),
            Some("*".into()),
            None,
        )
        .unwrap();

        let reader = AixReader::new(output.data).unwrap();
        let ts_output = String::from_utf8(reader.read_file("scripts/types.ts").unwrap()).unwrap();
        assert_eq!(ts_output, "export const total:number=3;");

        let file_report = output
            .report
            .files
            .iter()
            .find(|file| file.path == "scripts/types.ts")
            .unwrap();
        assert_eq!(file_report.status, aix_pack::OptimizeStatus::Optimized);
    }
}
