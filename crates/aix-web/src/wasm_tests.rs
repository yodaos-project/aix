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
