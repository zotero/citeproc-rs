use crate::Fetcher;
use citeproc::prelude::*;
use csl::Lang;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", remote = "FormatOptions")]
pub(crate) struct JsFormatOptions {
    #[serde(default = "bool_true")]
    link_anchors: bool,
}

fn bool_true() -> bool {
    true
}

/// `remote = "FormatOptions` means it doesn't implement `DeserializeOwned`, which we need to use
/// `JsValue::into_serde()`. A wrapper works.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatOptionsArg (
    #[serde(with = "JsFormatOptions")]
    pub FormatOptions,
);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmInitOptions {
    // Required
    /// A full independent style.
    pub style: String,

    #[serde(default)]
    pub csl_features: Vec<String>,

    // Optional
    #[serde(default)]
    pub format: SupportedFormat,

    #[serde(default, with = "JsFormatOptions")]
    pub format_options: FormatOptions,

    /// You might get this from a dependent style via `StyleMeta::parse(dependent_xml_string)`
    #[serde(default)]
    pub locale_override: Option<Lang>,
    /// Disables sorting on the bibliography
    #[serde(default)]
    pub bibliography_no_sort: bool,
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
pub enum GetFetcherError {
    #[error("fetcher must be an object")]
    NotAnObject,
    #[error("fetcher object must contain an `async fetchLocale(lang: string): string` function")]
    MissingFetchLocale,
}

impl Fetcher {
    /// You can't deserialize a JsValue from a generic Deserialize impl, as that's meant to work
    /// with any serialization format. So we pull this off the options object separately. Pass in a
    /// JsValue with the structure `{ fetcher?: Fetcher }`. If it's missing the fetcher
    /// field, you get Ok(None).
    pub fn from_options_object(options: &JsValue) -> Result<Option<Self>, GetFetcherError> {
        let object = Object::from(options.clone());
        thread_local! {
            static FETCHER_FIELD: JsValue = JsValue::from_str("fetcher");
            static FETCH_LOCALE_FIELD: JsValue = JsValue::from_str("fetchLocale")
        }
        let jsvalue = FETCHER_FIELD.with(|field| object.get(field.clone()));
        let fetcher_obj = Object::from(jsvalue.clone());
        if jsvalue.is_undefined() {
            return Ok(None);
        }
        if !jsvalue.is_object() {
            return Err(GetFetcherError::NotAnObject);
        }
        let fetch_locale = FETCH_LOCALE_FIELD.with(|f| fetcher_obj.get(f.clone()));
        if !fetch_locale.is_function() {
            return Err(GetFetcherError::MissingFetchLocale);
        }
        let fetcher = Fetcher::from(jsvalue);
        Ok(Some(fetcher))
    }
}


