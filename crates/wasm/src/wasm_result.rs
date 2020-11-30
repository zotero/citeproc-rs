pub use helper::*;

pub trait TypescriptResult: From<WasmResult> {
    type RustType;
}

macro_rules! result_type {
  ($ty:ty, $name:ident, $stringified:literal) => {
    #[wasm_bindgen]
    extern "C" {
      #[wasm_bindgen(typescript_type = $stringified)]
      pub type $name;
    }
    impl From<$crate::wasm_result::WasmResult> for $name {
      fn from(other: $crate::wasm_result::WasmResult) -> Self {
        let jsv: JsValue = other.into();
        $name::from(jsv)
      }
    }
    impl TypescriptResult for $name {
        type RustType = $ty;
    }
  };
}

mod helper {
    use wasm_bindgen::prelude::*;
    use js_sys::Error as JsError;
    use serde::Serialize;
    use super::TypescriptResult;
    use crate::{DriverError, CiteprocRsDriverError, CiteprocRsError, CslStyleError};
    use csl::StyleError;

    crate::js_import_class_constructor! {
        pub type WasmResult;
        #[wasm_bindgen(constructor)]
        fn new(value: JsValue) -> WasmResult;
    }

    pub fn js_value_err<V, E, F>(f: F) -> WasmResult
    where
        V: Into<JsValue>,
        E: std::error::Error,
        F: FnOnce() -> Result<V, E>,
    {
        let res = f();
        let out = match res {
            Ok(ok) => {
                WasmResult::new(ok.into())
            }
            Err(e) => {
                WasmResult::new(JsError::new(&e.to_string()).into())
            }
        };
        out
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
            },
        }
    }

    impl DriverError {
        fn to_js_error(&self) -> JsValue {
            match self {
                DriverError::StyleError(se) => return style_error_to_js_err(se),
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
                },
            }
        }
    }

    impl From<Result<JsValue, DriverError>> for WasmResult {
        fn from(res: Result<JsValue, DriverError>) -> Self {
            match res {
                Ok(ok) => {
                    WasmResult::new(ok.into())
                }
                Err(e) => {
                    WasmResult::new(e.to_js_error())
                }
            }
        }
    }

    pub fn js_driver_error<V, F>(f: F) -> WasmResult
    where
        V: Into<JsValue>,
        F: FnOnce() -> Result<V, DriverError>,
    {
        f().map(|x| x.into()).into()
    }

    pub fn typescript_serde_result<R, F>(f: F) -> R
    where
        R: TypescriptResult,
        R::RustType: Serialize,
        F: FnOnce() -> Result<R::RustType, DriverError>,
    {
        let res = f()
            .and_then(|rust| {
                JsValue::from_serde(&rust)
                    .map_err(DriverError::from)
            });
        let out: WasmResult = res.into();
        out.into()
    }

}

mod raw {
    //! Alternative impl with no Javascript dependency but no methods available
    //! on the returned objects.

    use wasm_bindgen::prelude::*;
    use js_sys::Error as JsError;
    use serde::Serialize;

    pub type WasmResult = JsValue;

    // From serde_wasm_bindgen
    /// Custom bindings to avoid using fallible `Reflect` for plain objects.
    #[wasm_bindgen]
    extern "C" {
        pub type Object;

        #[wasm_bindgen(constructor)]
        pub fn new() -> Object;

        #[wasm_bindgen(method, indexing_setter)]
        pub fn set(this: &Object, key: JsValue, value: JsValue);
    }

    thread_local! {
        pub static OK_FIELD: JsValue = JsValue::from_str("Ok");
        pub static ERR_FIELD: JsValue = JsValue::from_str("Err");
    }

    pub fn js_value_err<E, F>(f: F) -> JsValue
    where
        E: std::error::Error,
        F: FnOnce() -> Result<JsValue, E>,
    {
        let res = f();
        let out = Object::new();
        match res {
            Ok(ok) => {
                out.set(OK_FIELD.with(|f| f.clone()), ok);
            }
            Err(e) => {
                out.set(
                    ERR_FIELD.with(|f| f.clone()),
                    JsError::new(&e.to_string()).into(),
                );
            }
        }
        out.into()
    }

    pub fn js_serde_err<T, E, F>(f: F) -> JsValue
    where
        T: Serialize,
        E: std::error::Error,
        F: FnOnce() -> Result<T, E>,
    {
        let res = f();
        let out = Object::new();
        match res {
            Ok(ok) => {
                let value = JsValue::from_serde(&ok)
                    .expect("citeproc-wasm failed to serialize return value to JsValue");
                out.set(OK_FIELD.with(|f| f.clone()), value);
            }
            Err(e) => {
                out.set(
                    ERR_FIELD.with(|f| f.clone()),
                    JsError::new(&e.to_string()).into(),
                );
            }
        }
        out.into()
    }
}

