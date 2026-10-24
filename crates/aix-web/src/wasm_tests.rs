use super::*;
use wasm_bindgen_test::wasm_bindgen_test;

fn files() -> Vec<InputFile> {
    vec![
        InputFile::new("app.json", br#"{"pages":["home"],"widgets":[{"path":"card","family":"1x1"}],"window":{"navigationBarTitleText":"Example"}}"#),
        InputFile::new("home.ink", b"<script def>{\"schema\":{\"data\":{\"type\":\"object\"}}}</script><page><view /></page>"),
        InputFile::new("card.ink", b"<script def>{\"widget\":{\"family\":\"1x1\"}}</script><widget><view /></widget>"),
        InputFile::new("app.en-US.json", br#"{"widgets":{"card":{"displayName":"Clock"}}}"#),
    ]
}

#[wasm_bindgen_test]
fn source_snapshots_normalize_files_and_propagate_errors() {
    let inputs = vec![
        InputFile::new(".aixignore", b"ignored/"),
        InputFile::new("ignored/bad.json", b"{broken"),
        InputFile::new("nested/.aix/cache", b"local cache"),
        InputFile::new("pages\\home.ink", b"\xef\xbb\xbf<view />"),
    ];
    let snapshot: Vec<InputFile> =
        serde_wasm_bindgen::from_value(collect_aix_source(to_value(&inputs).unwrap()).unwrap())
            .unwrap();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].path, "pages/home.ink");
    assert_eq!(snapshot[0].data, b"<view />");
    assert!(collect_aix_source(JsValue::NULL).is_err());
    for (inputs, message) in [
        (
            vec![InputFile::new(".aixignore", b"[z-a]")],
            "Invalid .aixignore",
        ),
        (
            vec![InputFile::new("bad.ink", b"\xff\xfe\x00")],
            "Failed to convert bad.ink",
        ),
    ] {
        let error = collect_aix_source(to_value(&inputs).unwrap()).unwrap_err();
        assert!(error.as_string().unwrap().contains(message));
    }
}

fn assert_package(output: AixPackResultWasm) {
    let data = output.data().to_vec();
    assert_eq!(data, output.data);
    let report: aix_pack::OptimizeReport =
        serde_wasm_bindgen::from_value(output.report().unwrap()).unwrap();
    assert!(report.files.iter().any(|file| file.path == "home.ink"));
    let warnings: Vec<String> = serde_wasm_bindgen::from_value(output.warnings().unwrap()).unwrap();
    assert!(warnings.is_empty());
    let reader = AixReaderWasm::new(data).unwrap();
    assert_eq!(reader.get_version().as_deref(), Some("build"));
    assert_eq!(reader.get_title().as_deref(), Some("Example"));
    assert!(reader.supports_engine("0.18.1").unwrap());
    assert!(reader.supports_engine("invalid").is_err());
    assert!(reader.read_file("app.json").is_ok());
    assert!(reader.read_file("missing").is_err());
    let entries: Vec<serde_json::Value> =
        serde_wasm_bindgen::from_value(reader.list().unwrap()).unwrap();
    assert!(entries.len() >= 5);
    let pages: Vec<serde_json::Value> =
        serde_wasm_bindgen::from_value(reader.get_pages().unwrap()).unwrap();
    assert_eq!(pages[0]["name"], "home");
    let tools: Vec<serde_json::Value> =
        serde_wasm_bindgen::from_value(reader.get_tools().unwrap()).unwrap();
    assert_eq!(tools[0]["function"]["name"], "home");
    for locale in [None, Some("en_US".into())] {
        let widgets: Vec<serde_json::Value> =
            serde_wasm_bindgen::from_value(reader.get_widgets(locale.clone()).unwrap()).unwrap();
        assert_eq!(widgets[0]["path"], "card");
        if locale.is_some() {
            assert_eq!(widgets[0]["displayName"], "Clock");
        }
    }
}

#[wasm_bindgen_test]
fn metadata_diagnostics_cross_the_wasm_boundary_without_hiding_valid_pages() {
    let inputs = vec![
        InputFile::new("app.json", br#"{"pages":["missing","good","bad"],"widgets":[{"path":"card","family":"1x1","placement":"invalid"}]}"#),
        InputFile::new("good.ink", br#"<script def>{"navigationBarTitleText":"Good"}</script><page><view /></page>"#),
        InputFile::new("bad.ink", br#"<script def>{"schema":false}</script><page><view /></page>"#),
    ];
    let output = pack_output(inputs, "metadata".into(), None, None).unwrap();
    let reader = AixReaderWasm::new(output.data).unwrap();
    let report: aix::MetadataResolution =
        serde_wasm_bindgen::from_value(reader.resolve_metadata().unwrap()).unwrap();
    assert_eq!(report.pages.len(), 1);
    assert_eq!(report.pages[0].name, "good");
    assert_eq!(report.tools[0].function.name, "good");
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|d| d.path.as_str())
            .collect::<Vec<_>>(),
        ["missing.json", "bad.ink"]
    );
    let legacy: Vec<serde_json::Value> =
        serde_wasm_bindgen::from_value(reader.get_pages().unwrap()).unwrap();
    assert_eq!(legacy.len(), 3);
    assert!(reader.get_widgets(None).is_err());
    for (app, diagnostic_count) in [
        (br#"{"pages":[]}"#.as_slice(), 0),
        (br#"{"pages":false}"#, 1),
    ] {
        let output = pack_output(
            vec![InputFile::new("app.json", app)],
            "metadata".into(),
            None,
            None,
        )
        .unwrap();
        let reader = AixReaderWasm::new(output.data).unwrap();
        let report: aix::MetadataResolution =
            serde_wasm_bindgen::from_value(reader.resolve_metadata().unwrap()).unwrap();
        assert!(report.pages.is_empty());
        assert!(report.tools.is_empty());
        assert_eq!(report.diagnostics.len(), diagnostic_count);
    }
}

#[wasm_bindgen_test]
fn reader_limits_reject_metadata_budgets_and_invalid_javascript_values() {
    let output = pack_output(
        vec![
            InputFile::new("app.json", br#"{"pages":[]}"#),
            InputFile::new("payload", &[42; 4096]),
        ],
        "limits".into(),
        None,
        None,
    )
    .unwrap();
    let entries = AixReader::new(output.data.clone()).unwrap().list().to_vec();
    let total: u64 = entries.iter().map(|entry| entry.size).sum();
    let limits = serde_json::json!({
        "maxEntries": entries.len(), "maxEntryBytes": 4096, "maxTotalBytes": total,
    });
    let reader =
        AixReaderWasm::with_limits(output.data.clone(), to_value(&limits).unwrap()).unwrap();
    assert_eq!(reader.read_file("payload").unwrap(), vec![42; 4096]);
    for (limits, message) in [
        (
            serde_json::json!({"maxEntries": entries.len() - 1}),
            "entry count limit",
        ),
        (
            serde_json::json!({"maxEntryBytes": 4095}),
            "per-entry byte limit",
        ),
        (
            serde_json::json!({"maxTotalBytes": total - 1}),
            "total byte limit",
        ),
        (serde_json::json!({"maxEntries": -1}), "Invalid read limits"),
        (
            serde_json::json!({"maxEntryBytes": 0.5}),
            "Invalid read limits",
        ),
        (
            serde_json::json!({"maxTotalBytes": 9007199254740992.0}),
            "Invalid read limits",
        ),
        (
            serde_json::json!({"maxEntryByte": 1}),
            "Invalid read limits",
        ),
    ] {
        let error = AixReaderWasm::with_limits(output.data.clone(), to_value(&limits).unwrap())
            .err()
            .unwrap();
        assert!(error.as_string().unwrap().contains(message));
    }
    assert!(AixReaderWasm::with_limits(output.data.clone(), JsValue::NULL).is_err());
    assert!(AixReaderWasm::with_limits(output.data.clone(), JsValue::TRUE).is_err());
    assert!(AixReaderWasm::with_limits(output.data.clone(), js_sys::Array::new().into()).is_err());
    assert!(
        AixReaderWasm::with_limits(output.data, to_value(&serde_json::json!({})).unwrap()).is_ok()
    );
}

#[wasm_bindgen_test]
fn javascript_pack_and_reader_surfaces_round_trip() {
    let options = to_value(&OptimizeOptions::default()).unwrap();
    for optimize in [JsValue::NULL, JsValue::UNDEFINED, options] {
        assert_package(
            pack_aix(
                to_value(&files()).unwrap(),
                "build".into(),
                Some("*".into()),
                optimize.clone(),
            )
            .unwrap(),
        );
        assert_package(
            pack_aix_from_source(
                to_value(&files()).unwrap(),
                "build".into(),
                None,
                optimize.clone(),
            )
            .unwrap(),
        );
        let output = pack_aix_from_source_with_progress(
            to_value(&files()).unwrap(),
            "build".into(),
            None,
            optimize.clone(),
            Function::new_no_args("return;"),
        )
        .unwrap();
        assert_package(optimize_aix(output.data, optimize).unwrap());
    }
    let mut builder = AixSourcePackBuilderWasm::default();
    for file in files() {
        builder.add_file(file.path, file.data);
    }
    assert_package(
        builder
            .pack_from_source_with_progress(
                "build".into(),
                None,
                JsValue::NULL,
                Function::new_no_args("return;"),
            )
            .unwrap(),
    );
    assert!(
        builder.files.is_empty(),
        "builder must release transferred inputs"
    );
    assert!(builder
        .pack_from_source_with_progress(
            "build".into(),
            None,
            JsValue::NULL,
            Function::new_no_args("return;")
        )
        .is_err());
}

#[wasm_bindgen_test]
fn checks_return_plain_javascript_objects_and_diagnostics() {
    let inputs = to_value(&files()).unwrap();
    let report: serde_json::Value =
        serde_wasm_bindgen::from_value(check_aix_from_source(inputs.clone()).unwrap()).unwrap();
    assert_eq!(report["diagnostics"], serde_json::json!([]));
    let prepared: serde_json::Value =
        serde_wasm_bindgen::from_value(prepare_aix_check(inputs).unwrap()).unwrap();
    assert_eq!(prepared["report"], report);
    assert_eq!(prepared["coverage"]["schemas"], 1);
    assert_eq!(prepared["coverage"]["structure"], true);
    for invalid in [
        JsValue::NULL,
        to_value(&vec![InputFile::new("a/../b", b"bad")]).unwrap(),
    ] {
        assert!(check_aix_from_source(invalid.clone()).is_err());
        assert!(prepare_aix_check(invalid).is_err());
    }
}

#[wasm_bindgen_test]
fn javascript_boundary_errors_and_callback_failures_are_propagated() {
    let callback = || {
        Function::new_with_args(
            "event",
            "if (event.type === 'preparing_files') throw new Error('cancelled');",
        )
    };
    assert!(AixReaderWasm::new(b"invalid zip".to_vec()).is_err());
    assert!(optimize_aix(vec![], JsValue::NULL).is_err());
    assert!(optimize_aix(vec![], JsValue::TRUE).is_err());
    for inputs in [JsValue::NULL, to_value(&Vec::<InputFile>::new()).unwrap()] {
        assert!(pack_aix(inputs.clone(), "build".into(), None, JsValue::NULL).is_err());
        assert!(pack_aix_from_source(inputs.clone(), "build".into(), None, JsValue::NULL).is_err());
        assert!(pack_aix_from_source_with_progress(
            inputs,
            "build".into(),
            None,
            JsValue::NULL,
            callback()
        )
        .is_err());
    }
    assert!(pack_aix(
        to_value(&files()).unwrap(),
        "build".into(),
        None,
        JsValue::TRUE
    )
    .is_err());
    assert!(pack_aix_from_source(
        to_value(&files()).unwrap(),
        "build".into(),
        None,
        JsValue::TRUE
    )
    .is_err());
    assert!(pack_aix_from_source_with_progress(
        to_value(&files()).unwrap(),
        "build".into(),
        None,
        JsValue::TRUE,
        callback()
    )
    .is_err());
    assert!(pack_aix_from_source_with_progress(
        to_value(&files()).unwrap(),
        "build".into(),
        None,
        JsValue::NULL,
        callback()
    )
    .is_err());
    assert!(pack_aix_from_source_with_progress(
        to_value(&files()).unwrap(),
        "build".into(),
        None,
        JsValue::NULL,
        Function::new_no_args("throw new Error('transfer failed')")
    )
    .is_err());
    let mut builder = AixSourcePackBuilderWasm::new();
    for file in files() {
        builder.add_file(file.path, file.data);
    }
    assert!(builder
        .pack_from_source_with_progress("build".into(), None, JsValue::TRUE, callback())
        .is_err());
    assert!(
        !builder.files.is_empty(),
        "invalid options must not consume inputs"
    );
    assert!(builder
        .pack_from_source_with_progress("build".into(), None, JsValue::NULL, callback())
        .is_err());
    let output = pack_output(
        vec![InputFile::new(
            "app.json",
            br#"{"pages":[],"widgets":[{"path":"missing","family":"1x1"}]}"#,
        )],
        "build".into(),
        None,
        None,
    )
    .unwrap();
    let reader = AixReaderWasm::new(output.data).unwrap();
    assert!(reader.get_widgets(None).is_err());
    assert!(reader.get_widgets(Some("en-US".into())).is_err());
    struct Invalid;
    impl serde::Serialize for Invalid {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("invalid serializer input"))
        }
    }
    assert!(to_value(&Invalid).is_err());
}
