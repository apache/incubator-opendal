// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use std::collections::HashMap;
use std::os::raw::c_char;

use crate::error::{ErrorCode, OpenDALError};

pub fn cstr_to_str<'a>(value: *const c_char) -> Option<&'a str> {
    if value.is_null() {
        return None;
    }

    let cstr = unsafe { std::ffi::CStr::from_ptr(value) };
    cstr.to_str().ok()
}

pub fn config_invalid_error(message: impl Into<String>) -> OpenDALError {
    OpenDALError::from_error(ErrorCode::ConfigInvalid, message.into())
}

pub fn invalid_utf8_message(field: &str) -> String {
    format!("{field} is null or invalid UTF-8")
}

pub fn require_cstr<'a>(value: *const c_char, field: &str) -> Result<&'a str, OpenDALError> {
    cstr_to_str(value).ok_or_else(|| {
        OpenDALError::from_error(ErrorCode::ConfigInvalid, invalid_utf8_message(field))
    })
}

pub fn require_op_handle<'a>(
    op_handle: *const crate::operator::OperatorHandle,
) -> Result<&'a crate::operator::OperatorHandle, OpenDALError> {
    if op_handle.is_null() {
        return Err(config_invalid_error("operator handle is null"));
    }

    Ok(unsafe { &*op_handle })
}

pub fn require_callback<T>(callback: Option<T>) -> Result<T, OpenDALError> {
    callback.ok_or_else(|| config_invalid_error("callback pointer is null"))
}

pub fn require_data_ptr(data: *const u8, len: usize) -> Result<(), OpenDALError> {
    if len > 0 && data.is_null() {
        return Err(config_invalid_error("data pointer is null while len > 0"));
    }

    Ok(())
}

pub fn into_string_ptr(message: impl Into<String>) -> *mut c_char {
    match std::ffi::CString::new(message.into()) {
        Ok(msg) => msg.into_raw(),
        Err(_) => std::ffi::CString::new("invalid error message")
            .unwrap()
            .into_raw(),
    }
}

/// Discriminant that carries `opendal::EntryMode` across the FFI boundary.
pub fn entry_mode_code(mode: opendal::EntryMode) -> i32 {
    match mode {
        opendal::EntryMode::FILE => 0,
        opendal::EntryMode::DIR => 1,
        opendal::EntryMode::Unknown => 2,
    }
}

/// Convert an optional string into an owned UTF-8 C string pointer, or null.
pub fn optional_c_string(value: Option<&str>) -> *mut c_char {
    value.map(into_string_ptr).unwrap_or(std::ptr::null_mut())
}

/// Release a C string produced by `into_string_ptr` and null the field.
///
/// # Safety
///
/// - `field` must be null or produced by `into_string_ptr`.
/// - Must be called at most once for the same pointer.
pub unsafe fn release_c_string(field: &mut *mut c_char) {
    if field.is_null() {
        return;
    }

    drop(unsafe { std::ffi::CString::from_raw(*field) });
    *field = std::ptr::null_mut();
}

/// Flatten `Option<bool>` into a presence byte and a value byte.
pub fn optional_bool(value: Option<bool>) -> (u8, u8) {
    match value {
        Some(value) => (1, u8::from(value)),
        None => (0, 0),
    }
}

/// Flatten `Option<Timestamp>` into a presence byte, Unix seconds and
/// nanoseconds.
pub fn optional_timestamp(value: Option<opendal::raw::Timestamp>) -> (u8, i64, i32) {
    match value {
        Some(value) => {
            let value = value.into_inner();
            (1, value.as_second(), value.subsec_nanosecond())
        }
        None => (0, 0, 0),
    }
}

/// Flatten optional string pairs into a presence byte plus parallel key and
/// value arrays of `len` owned C strings.
///
/// The presence byte is `1` when the source reported a value, even an empty
/// one. The arrays are boxed slices of exactly `len` elements, or null when
/// there are no pairs, so `release_string_pairs` can rebuild them from the
/// raw parts alone.
pub fn string_pairs<'a>(
    pairs: Option<impl IntoIterator<Item = (&'a str, &'a str)>>,
) -> (u8, *mut *mut c_char, *mut *mut c_char, usize) {
    let Some(pairs) = pairs else {
        return (0, std::ptr::null_mut(), std::ptr::null_mut(), 0);
    };

    let mut keys = Vec::new();
    let mut values = Vec::new();
    for (key, value) in pairs {
        keys.push(into_string_ptr(key));
        values.push(into_string_ptr(value));
    }

    let len = keys.len();
    if len == 0 {
        return (1, std::ptr::null_mut(), std::ptr::null_mut(), 0);
    }

    let keys = Box::into_raw(keys.into_boxed_slice()) as *mut *mut c_char;
    let values = Box::into_raw(values.into_boxed_slice()) as *mut *mut c_char;
    (1, keys, values, len)
}

/// Release both arrays produced by `string_pairs` and their strings, leaving
/// the arrays null and the length zero.
///
/// # Safety
///
/// - `keys` and `values` must be null or produced by `string_pairs` together
///   with the same `len`.
/// - Must be called at most once for the same arrays.
pub unsafe fn release_string_pairs(
    keys: &mut *mut *mut c_char,
    values: &mut *mut *mut c_char,
    len: &mut usize,
) {
    for array in [keys, values] {
        if array.is_null() {
            continue;
        }

        let items = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(*array, *len)) };
        for item in items.iter() {
            if !item.is_null() {
                drop(unsafe { std::ffi::CString::from_raw(*item) });
            }
        }
        *array = std::ptr::null_mut();
    }
    *len = 0;
}

/// Copy a borrowed C string into an owned `String`, where null is `None`.
///
/// `value` must be null or a valid null-terminated string.
pub fn borrowed_string(value: *const c_char, field: &str) -> Result<Option<String>, OpenDALError> {
    if value.is_null() {
        return Ok(None);
    }

    cstr_to_str(value)
        .map(|value| Some(value.to_owned()))
        .ok_or_else(|| config_invalid_error(invalid_utf8_message(field)))
}

/// Rebuild a timestamp flattened by `optional_timestamp`.
pub fn borrowed_timestamp(
    has_value: u8,
    second: i64,
    nanosecond: i32,
) -> Result<Option<opendal::raw::Timestamp>, OpenDALError> {
    if has_value == 0 {
        return Ok(None);
    }

    opendal::raw::Timestamp::new(second, nanosecond)
        .map(Some)
        .map_err(OpenDALError::from_opendal_error)
}

/// Copy borrowed parallel arrays of `len` key and value C strings into a map.
///
/// A null value is reported by key, so callers can tell which entry was
/// unset. When `len > 0`, `keys` and `values` must be valid arrays of `len`
/// pointers, each null or a valid null-terminated string.
pub fn borrowed_string_map(
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
    field: &str,
) -> Result<HashMap<String, String>, OpenDALError> {
    if len == 0 {
        return Ok(HashMap::new());
    }
    if keys.is_null() || values.is_null() {
        return Err(config_invalid_error(format!(
            "{field} arrays are null while len > 0"
        )));
    }

    let keys = unsafe { std::slice::from_raw_parts(keys, len) };
    let values = unsafe { std::slice::from_raw_parts(values, len) };
    let mut map = HashMap::with_capacity(len);
    for (&key, &value) in keys.iter().zip(values) {
        let key = require_cstr(key, &format!("{field} key"))?;
        if value.is_null() {
            return Err(config_invalid_error(format!(
                "value of {field} {key} is null"
            )));
        }
        let value = require_cstr(value, &format!("value of {field} {key}"))?;
        map.insert(key.to_owned(), value.to_owned());
    }
    Ok(map)
}

/// Rebuild optional string pairs flattened by `string_pairs`, borrowing the
/// arrays as `borrowed_string_map` does.
pub fn borrowed_string_pairs(
    has_value: u8,
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
    field: &str,
) -> Result<Option<HashMap<String, String>>, OpenDALError> {
    if has_value == 0 {
        return Ok(None);
    }

    borrowed_string_map(keys, values, len, field).map(Some)
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::*;

    #[test]
    fn optional_bool_flattens_presence_and_value() {
        assert_eq!(optional_bool(Some(true)), (1, 1));
        assert_eq!(optional_bool(Some(false)), (1, 0));
        assert_eq!(optional_bool(None), (0, 0));
    }

    #[test]
    fn optional_timestamp_without_value_is_zeroed() {
        assert_eq!(optional_timestamp(None), (0, 0, 0));
    }

    #[test]
    fn string_pairs_round_trip_and_release() {
        let (has_value, mut keys, mut values, mut len) =
            string_pairs(Some([("a", "1"), ("b", "2")]));
        assert_eq!((has_value, len), (1, 2));

        let read = |array: *mut *mut c_char| -> Vec<&str> {
            (0..len)
                .map(|i| cstr_to_str(unsafe { *array.add(i) }).unwrap())
                .collect()
        };
        assert_eq!(read(keys), ["a", "b"]);
        assert_eq!(read(values), ["1", "2"]);

        unsafe { release_string_pairs(&mut keys, &mut values, &mut len) };
        assert!(keys.is_null());
        assert!(values.is_null());
        assert_eq!(len, 0);
    }

    #[test]
    fn string_pairs_distinguish_absent_from_empty() {
        let (has_value, keys, _, len) = string_pairs(None::<Vec<(&str, &str)>>);
        assert_eq!((has_value, len), (0, 0));
        assert!(keys.is_null());

        let (has_value, keys, _, len) = string_pairs(Some(Vec::<(&str, &str)>::new()));
        assert_eq!((has_value, len), (1, 0));
        assert!(keys.is_null());
    }

    #[test]
    fn borrowed_string_map_copies_and_names_null_values() {
        let key = CString::new("root").unwrap();
        let value = CString::new("/tmp").unwrap();
        let keys = [key.as_ptr()];
        let values = [value.as_ptr()];
        let map = borrowed_string_map(keys.as_ptr(), values.as_ptr(), 1, "option").unwrap();
        assert_eq!(map["root"], "/tmp");

        let nulls = [std::ptr::null()];
        let err = borrowed_string_map(keys.as_ptr(), nulls.as_ptr(), 1, "option").unwrap_err();
        let message = cstr_to_str(err.message).unwrap();
        assert!(message.contains("value of option root is null"));
    }
}
