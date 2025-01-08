use wasm_bindgen::JsValue;

use crate::Error;
use crate::ErrorKind;

impl From<JsValue> for Error {
    fn from(value: JsValue) -> Self {
        Error::new(ErrorKind::Unexpected, "Error")
    }
}
