use aix::AixReader;
use aix_pack::{collector::CollectOptions, InputFile, OptimizeOptions, PackOptions};
use anyhow::Result;
use js_sys::Function;
use wasm_bindgen::prelude::*;

fn to_value<T: serde::Serialize + ?Sized>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true))
        .map_err(|e| JsValue::from_str(&e.to_string()))
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
    #[wasm_bindgen(constructor)]
    pub fn new(data: Vec<u8>) -> Result<AixReaderWasm, JsValue> {
        let inner = AixReader::new(data).map_err(|e| JsValue::from_str(&e.to_string()))?;
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
        to_value(&self.inner.get_pages())
    }

    pub fn get_widgets(&self, locale: Option<String>) -> Result<JsValue, JsValue> {
        let widgets = match locale.as_deref() {
            Some(locale) => self.inner.get_widgets_for_locale(locale),
            None => self.inner.get_widgets(),
        }
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
        to_value(&widgets)
    }

    pub fn get_tools(&self) -> Result<JsValue, JsValue> {
        to_value(&self.inner.get_tools())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
