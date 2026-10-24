use aix::{AixReadLimits, AixReader};
use anyhow::Result;
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

#[cfg(feature = "tooling")]
mod tooling;
#[cfg(feature = "tooling")]
pub use tooling::*;

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
