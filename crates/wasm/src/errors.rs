use citeproc::string_id;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;

use csl::StyleError;

use crate::options::GetFetcherError;

/// Enum representing all the errors we throw in citeproc-rs.
///
/// Serialized as CiteprocRsError, or a subclass thereof.
#[derive(thiserror::Error, Debug, serde::Serialize)]
#[serde(tag = "tag", content = "content")]
pub enum Error {
    /// Serialized as CslStyleError
    #[error("Style error: {0}")]
    StyleError(#[from] csl::StyleError),

    // The rest are serialized as CiteprocRsDriverError.

    #[error("Unknown output format {0:?}")]
    UnknownOutputFormat(String),
    #[error("Unknown CSL feature {0:?}")]
    UnknownCSLFeature(String),
    #[error("JSON Deserialization Error: {0}")]
    JsonError(
        #[from]
        #[serde(skip_serializing)]
        serde_json::Error,
    ),
    #[error("Invalid fetcher object: {0}")]
    GetFetcherError(#[from] GetFetcherError),
    #[error("Non-Existent Cluster id: {0}")]
    NonExistentCluster(String),
    #[error("Reordering error: {0}")]
    ReorderingError(
        #[from]
        #[serde(skip_serializing)]
        string_id::ReorderingError,
    ),

    // This should not be necessary
    #[error("Reordering error: {0}")]
    ReorderingErrorNumericId(
        #[from]
        #[serde(skip_serializing)]
        citeproc::ReorderingError,
    ),
}

fn style_error_to_js_err(se: &StyleError) -> JsValue {
    let mut string = se.to_string();
    let data = JsValue::from_serde(&se);
    match data {
        Ok(data) => CslStyleError::new(string.into(), data).into(),
        Err(conv_err) => {
            string.push_str(" (could not convert error data: ");
            string.push_str(&conv_err.to_string());
            string.push_str(") ");
            CiteprocRsError::new(string.into()).into()
        }
    }
}

impl Error {
    fn to_js_error(&self) -> JsValue {
        match self {
            Error::StyleError(se) => return style_error_to_js_err(se),
            _ => {}
        }
        let mut string = self.to_string();
        let data = JsValue::from_serde(&self);
        match data {
            Ok(data) => CiteprocRsDriverError::new(string.into(), data).into(),
            Err(conv_err) => {
                string.push_str(" (could not convert error data: ");
                string.push_str(&conv_err.to_string());
                string.push_str(") ");
                CiteprocRsError::new(string.into()).into()
            }
        }
    }
}

impl From<Error> for JsValue {
    fn from(e: Error) -> Self {
        e.to_js_error()
    }
}

js_import_class_constructor! {
    pub type CiteprocRsError;
    #[wasm_bindgen(constructor)]
    fn new(msg: JsValue) -> CiteprocRsError;
}

js_import_class_constructor! {
    pub type CiteprocRsDriverError;
    #[wasm_bindgen(constructor)]
    fn new(msg: JsValue, data: JsValue) -> CiteprocRsDriverError;
}

js_import_class_constructor! {
    pub type CslStyleError;
    #[wasm_bindgen(constructor)]
    fn new(msg: JsValue, data: JsValue) -> CslStyleError;
}

#[wasm_bindgen(typescript_custom_section)]
const TS_APPEND_CONTENT_5: &'static str = r#"
type Severity = "Error" | "Warning";
interface InvalidCsl {
    severity: Severity;
    /** Relevant bytes in the provided XML */
    range: {
        start: number,
        end: number,
    };
    message: string;
    hint: string | undefined;
}
type StyleError = {
    tag: "Invalid",
    content: InvalidCsl[],
} | {
    tag: "ParseError",
    content: string,
} | {
    /** Cannot use a dependent style to format citations, pass the parent style instead. */
    tag: "DependentStyle",
    content: {
        requiredParent: string,
    }
};
type DriverError = {
    tag: "UnknownOutputFormat",
    content: string,
} | {
    tag: "JsonError",
} | {
    tag: "GetFetcherError",
} | {
    tag: "NonExistentCluster",
    content: string,
} | {
    tag: "ReorderingError"
} | {
    tag: "ReorderingErrorNumericId"
};

declare global {
    /** Catch-all citeproc-rs Error subclass.
     * 
     * CiteprocRsDriverError and CslStyleError are both subclasses of this, so checking instanceof
     * CiteprocRsError suffices to catch all errors thrown by @citeproc-rs/wasm itself.
     *
     * There may be errors that are not a subclass, but directly an instance of CitprocRsError, so
     * for completeness in a catch/instanceof check, one should test for this too.
     *
     * Using the library may also result in WASM runtime errors, and some generic Error objects
     * from e.g. using a Driver after you have called `.free()`.
     * */
    class CiteprocRsError extends Error {
        constructor(message: string);
    }
    /** Error in usage of Driver */
    class CiteprocRsDriverError extends CiteprocRsError {
        data: DriverError;
        constructor(message: string, data: DriverError);
    }
    /** Error parsing a CSL file */
    class CslStyleError extends CiteprocRsError {
        data: StyleError;
        constructor(message: string, data: StyleError);
    }
}
"#;

