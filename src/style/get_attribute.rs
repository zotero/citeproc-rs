use std::str::FromStr;
use crate::style::error::*;

pub trait GetAttribute where Self : Sized {
    fn get_attr(s: &str) -> Result<Self, UnknownAttributeValue>;
}

impl<T: FromStr> GetAttribute for T {
    fn get_attr(s: &str) -> Result<Self, UnknownAttributeValue> {
        match T::from_str(s) {
            Ok(a) => Ok(a),
            Err(e) => Err(UnknownAttributeValue::new(s))
        }
    }
}

