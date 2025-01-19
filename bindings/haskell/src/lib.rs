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

mod logger;
mod result;
mod types;

use std::collections::HashMap;
use std::ffi::CStr;
use std::mem;
use std::os::raw::c_char;
use std::str::FromStr;

use ::opendal as od;
use logger::HsLogger;
use od::layers::LoggingLayer;
use od::layers::RetryLayer;
use od::BlockingLister;
use result::FFIResult;
use types::ByteSlice;
use types::Metadata;

/// # Safety
///
/// * The `keys`, `values`, `len` are valid from `HashMap`.
/// * The memory pointed to by `scheme` contain a valid nul terminator at the end of
///   the string.
/// * The `result` is a valid pointer, and has available memory to write to.
///
/// # Panics
///
/// * If `keys` or `values` are not valid pointers.
/// * If `len` is not the same for `keys` and `values`.
/// * If `log_level` is not a valid value passed by haskell.
/// * If `callback` is not a valid function pointer.
/// * If `result` is not a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn via_map_ffi(
    scheme: *const c_char,
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
    callback: Option<extern "C" fn(u32, *const c_char)>,
    result: *mut FFIResult<od::BlockingOperator>,
) {
    let scheme_str = match CStr::from_ptr(scheme).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert scheme to string");
            return;
        }
    };

    let scheme = match od::Scheme::from_str(scheme_str) {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to parse scheme");
            return;
        }
    };

    let keys_vec = std::slice::from_raw_parts(keys, len);
    let values_vec = std::slice::from_raw_parts(values, len);

    let map = keys_vec
        .iter()
        .zip(values_vec.iter())
        .map(|(&k, &v)| {
            (
                CStr::from_ptr(k).to_string_lossy().into_owned(),
                CStr::from_ptr(v).to_string_lossy().into_owned(),
            )
        })
        .collect::<HashMap<String, String>>();

    if let Some(callback) = callback {
        if let Err(e) = log::set_boxed_logger(Box::new(HsLogger { callback }))
            .map(|()| log::set_max_level(log::LevelFilter::Debug))
        {
            *result = FFIResult::err_with_source(
                "Failed to register logger",
                od::Error::new(od::ErrorKind::Unexpected, e.to_string().as_str()),
            );
            return;
        }
    }

    let res = match od::Operator::via_iter(scheme, map) {
        Ok(mut operator) => {
            operator = operator.layer(RetryLayer::new());
            if callback.is_some() {
                operator = operator.layer(LoggingLayer::default());
            }

            FFIResult::ok(operator.blocking())
        }
        Err(e) => FFIResult::err_with_source("Failed to create Operator", e),
    };

    *result = res;
}

/// # Safety
///
/// * `operator` is a valid pointer to a `BlockingOperator`.
///
/// # Panics
///
/// * If `operator` is not a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn free_operator(operator: *mut od::BlockingOperator) {
    if !operator.is_null() {
        drop(Box::from_raw(operator));
    }
}

/// # Safety
///
/// * `op` is a valid pointer to a `BlockingOperator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn blocking_read(
    op: *mut od::BlockingOperator,
    path: *const c_char,
    result: *mut FFIResult<ByteSlice>,
) {
    let op = if op.is_null() {
        *result = FFIResult::err("Operator is null");
        return;
    } else {
        &mut *op
    };

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert path to string");
            return;
        }
    };

    let res = match op.read(path_str) {
        Ok(bytes) => FFIResult::ok(ByteSlice::from_vec(bytes.to_vec())),
        Err(e) => FFIResult::err_with_source("Failed to read", e),
    };

    *result = res;
}

/// # Safety
///
/// * `op` is a valid pointer to a `BlockingOperator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `bytes` is a valid pointer to a byte array.
/// * `len` is the length of `bytes`.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `bytes` is not a valid pointer, or `len` is more than the length of `bytes`.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn blocking_write(
    op: *mut od::BlockingOperator,
    path: *const c_char,
    bytes: *const c_char,
    len: usize,
    result: *mut FFIResult<()>,
) {
    let op = if op.is_null() {
        *result = FFIResult::err("Operator is null");
        return;
    } else {
        &mut *op
    };

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert path to string");
            return;
        }
    };

    let bytes = Vec::from_raw_parts(bytes as *mut u8, len, len);

    let res = match op.write(path_str, bytes.clone()) {
        Ok(_) => FFIResult::ok(()),
        Err(e) => FFIResult::err_with_source("Failed to write", e),
    };

    *result = res;

    // bytes memory is controlled by Haskell, we can't drop it here
    mem::forget(bytes);
}

/// # Safety
///
/// * `op` is a valid pointer to a `BlockingOperator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn blocking_is_exist(
    op: *mut od::BlockingOperator,
    path: *const c_char,
    result: *mut FFIResult<bool>,
) {
    let op = if op.is_null() {
        *result = FFIResult::err("Operator is null");
        return;
    } else {
        &mut *op
    };

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert path to string");
            return;
        }
    };

    let res = match op.is_exist(path_str) {
        Ok(exist) => FFIResult::ok(exist),
        Err(e) => FFIResult::err_with_source("Failed to check if path exists", e),
    };

    *result = res;
}

/// # Safety
///
/// * `op` is a valid pointer to a `BlockingOperator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn blocking_create_dir(
    op: *mut od::BlockingOperator,
    path: *const c_char,
    result: *mut FFIResult<()>,
) {
    let op = if op.is_null() {
        *result = FFIResult::err("Operator is null");
        return;
    } else {
        &mut *op
    };

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert path to string");
            return;
        }
    };

    let res = match op.create_dir(path_str) {
        Ok(()) => FFIResult::ok(()),
        Err(e) => FFIResult::err_with_source("Failed to create directory", e),
    };

    *result = res;
}

/// # Safety
///
/// * `op` is a valid pointer to a `BlockingOperator`.
/// * `path_from` is a valid pointer to a nul terminated string.
/// * `path_to` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn blocking_copy(
    op: *mut od::BlockingOperator,
    path_from: *const c_char,
    path_to: *const c_char,
    result: *mut FFIResult<()>,
) {
    let op = if op.is_null() {
        *result = FFIResult::err("Operator is null");
        return;
    } else {
        &mut *op
    };

    let path_from_str = match CStr::from_ptr(path_from).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert source path to string");
            return;
        }
    };

    let path_to_str = match CStr::from_ptr(path_to).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert destination path to string");
            return;
        }
    };

    let res = match op.copy(path_from_str, path_to_str) {
        Ok(()) => FFIResult::ok(()),
        Err(e) => FFIResult::err_with_source("Failed to copy", e),
    };

    *result = res;
}

/// # Safety
///
/// * `op` is a valid pointer to a `BlockingOperator`.
/// * `path_from` is a valid pointer to a nul terminated string.
/// * `path_to` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn blocking_rename(
    op: *mut od::BlockingOperator,
    path_from: *const c_char,
    path_to: *const c_char,
    result: *mut FFIResult<()>,
) {
    let op = if op.is_null() {
        *result = FFIResult::err("Operator is null");
        return;
    } else {
        &mut *op
    };

    let path_from_str = match CStr::from_ptr(path_from).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert source path to string");
            return;
        }
    };

    let path_to_str = match CStr::from_ptr(path_to).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert destination path to string");
            return;
        }
    };

    let res = match op.rename(path_from_str, path_to_str) {
        Ok(()) => FFIResult::ok(()),
        Err(e) => FFIResult::err_with_source("Failed to rename", e),
    };

    *result = res;
}

/// # Safety
///
/// * `op` is a valid pointer to a `BlockingOperator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn blocking_delete(
    op: *mut od::BlockingOperator,
    path: *const c_char,
    result: *mut FFIResult<()>,
) {
    let op = if op.is_null() {
        *result = FFIResult::err("Operator is null");
        return;
    } else {
        &mut *op
    };

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert path to string");
            return;
        }
    };

    let res = match op.delete(path_str) {
        Ok(()) => FFIResult::ok(()),
        Err(e) => FFIResult::err_with_source("Failed to delete", e),
    };

    *result = res;
}

/// # Safety
///
/// * `op` is a valid pointer to a `BlockingOperator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn blocking_stat(
    op: *mut od::BlockingOperator,
    path: *const c_char,
    result: *mut FFIResult<Metadata>,
) {
    let op = if op.is_null() {
        *result = FFIResult::err("Operator is null");
        return;
    } else {
        &mut *op
    };

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert path to string");
            return;
        }
    };

    let res = match op.stat(path_str) {
        Ok(meta) => FFIResult::ok(meta.into()),
        Err(e) => FFIResult::err_with_source("Failed to stat", e),
    };

    *result = res;
}

/// # Safety
///
/// * `op` is a valid pointer to a `BlockingOperator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn blocking_list(
    op: *mut od::BlockingOperator,
    path: *const c_char,
    result: *mut FFIResult<*mut BlockingLister>,
) {
    let op = if op.is_null() {
        *result = FFIResult::err("Operator is null");
        return;
    } else {
        &mut *op
    };

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert path to string");
            return;
        }
    };

    let res = match op.lister(path_str) {
        Ok(lister) => FFIResult::ok(Box::into_raw(Box::new(lister))),
        Err(e) => FFIResult::err_with_source("Failed to list", e),
    };

    *result = res;
}

/// # Safety
///
/// * `op` is a valid pointer to a `BlockingOperator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn blocking_scan(
    op: *mut od::BlockingOperator,
    path: *const c_char,
    result: *mut FFIResult<*mut BlockingLister>,
) {
    let op = if op.is_null() {
        *result = FFIResult::err("Operator is null");
        return;
    } else {
        &mut *op
    };

    let path_str = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => {
            *result = FFIResult::err("Failed to convert path to string");
            return;
        }
    };

    let res = match op.lister_with(path_str).recursive(true).call() {
        Ok(lister) => FFIResult::ok(Box::into_raw(Box::new(lister))),
        Err(e) => FFIResult::err_with_source("Failed to scan", e),
    };

    *result = res;
}

/// # Safety
///
/// * `lister` is a valid pointer to a `BlockingLister`.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `lister` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[no_mangle]
pub unsafe extern "C" fn lister_next(
    lister: *mut BlockingLister,
    result: *mut FFIResult<*const c_char>,
) {
    let lister = if lister.is_null() {
        *result = FFIResult::err("Lister is null");
        return;
    } else {
        &mut *lister
    };

    let res = match lister.next() {
        Some(Ok(item)) => {
            let res = types::leak_str(item.path());
            FFIResult::ok(res)
        }
        Some(Err(e)) => FFIResult::err_with_source("Failed to get next item", e),
        None => FFIResult::ok(std::ptr::null()),
    };

    *result = res;
}

/// # Safety
///
/// * `lister` is a valid pointer to a `BlockingLister`.
///
/// # Panics
///
/// * If `lister` is not a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn free_lister(lister: *mut BlockingLister) {
    if !lister.is_null() {
        drop(Box::from_raw(lister));
    }
}
