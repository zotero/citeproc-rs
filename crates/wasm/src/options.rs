use serde::{Deserialize, de::{Deserializer as _, MapAccess, IgnoredAny}};
use serde::de::Visitor;
use wasm_bindgen::prelude::*;
use citeproc::prelude::*;
use csl::Lang;
use crate::{Lifecycle, DriverError};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmInitOptions {
    // Required
    /// A full independent style.
    pub style: String,

    // Optional
    #[serde(default)]
    pub format: SupportedFormat,
    /// You might get this from a dependent style via `StyleMeta::parse(dependent_xml_string)`
    #[serde(default)]
    pub locale_override: Option<Lang>,
    /// Disables sorting on the bibliography
    #[serde(default)]
    pub bibliography_nosort: bool,
}


#[wasm_bindgen]
extern "C" {
    pub type Object;

    #[wasm_bindgen(constructor)]
    pub fn new() -> Object;

    #[wasm_bindgen(method, indexing_getter)]
    pub fn get(this: &Object, key: JsValue) -> JsValue;
}

#[derive(thiserror::Error, Debug, serde::Serialize)]
#[serde(tag = "tag", content = "content")]
pub enum GetLifecycleError {
    #[error("lifecycle must be an object")]
    NotAnObject,
    #[error("lifecycle object must contain an `async fetchLocale(lang: string): string` function")]
    MissingFetchLocale,
}

impl Lifecycle {
    /// You can't deserialize a JsValue from a generic Deserialize impl, as that's meant to work
    /// with any serialization format. So we pull this off the options object separately. Pass in a
    /// JsValue with the structure `{ lifecycle?: Lifecycle }`. If it's missing the lifecycle
    /// field, you get Ok(None).
    pub fn from_options_object(options: &JsValue) -> Result<Option<Self>, GetLifecycleError> {
        let object = Object::from(options.clone());
        thread_local! {
            static LIFECYCLE_FIELD: JsValue = JsValue::from_str("lifecycle");
            static FETCH_LOCALE_FIELD: JsValue = JsValue::from_str("fetchLocale")
        }
        let jsvalue = LIFECYCLE_FIELD.with(|field| object.get(field.clone()));
        let lifecycle_obj = Object::from(jsvalue.clone());
        if jsvalue.is_undefined() {
            return Ok(None);
        }
        if !jsvalue.is_object() {
            return Err(GetLifecycleError::NotAnObject);
        }
        let fetch_locale = FETCH_LOCALE_FIELD.with(|f| lifecycle_obj.get(f.clone()));
        if !fetch_locale.is_function() {
            return Err(GetLifecycleError::MissingFetchLocale);
        }
        let lifecycle = Lifecycle::from(jsvalue);
        Ok(Some(lifecycle))
    }
}

