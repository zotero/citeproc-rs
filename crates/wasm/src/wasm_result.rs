pub use helper::*;
use wasm_bindgen::JsValue;

pub trait TypescriptAlias {
    type RustType;
}

pub trait JsonValue: serde::Serialize {
    fn serialize_jsvalue<R: From<JsValue>>(&self) -> Result<R, crate::DriverError> {
        let jsvalue = JsValue::from_serde(self)?;
        Ok(jsvalue.into())
    }
}

impl<T> JsonValue for T where T: serde::Serialize {}

macro_rules! typescript_alias {
    ($ty:ty, $name:ident, $stringified:literal) => {
        #[wasm_bindgen]
        extern "C" {
            #[wasm_bindgen(typescript_type = $stringified)]
            pub type $name;
        }
        impl TypescriptAlias for $name {
            type RustType = $ty;
        }
    };
}

#[allow(dead_code)]
mod helper {
    use crate::{CiteprocRsDriverError, CiteprocRsError, CslStyleError, DriverError};
    use csl::StyleError;
    use wasm_bindgen::prelude::*;

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
                }
            }
        }
    }

    impl From<DriverError> for JsValue {
        fn from(e: DriverError) -> Self {
            e.to_js_error()
        }
    }
}
