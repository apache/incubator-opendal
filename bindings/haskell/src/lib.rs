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
use std::ffi::CString;
use std::mem;
use std::os::raw::c_char;
use std::sync::LazyLock;

use ::opendal as od;
use logger::HsLogger;
use od::layers::LoggingLayer;
use od::layers::RetryLayer;
use od::options;
use opendal::blocking;
use result::FFIResult;
use types::ByteSlice;
use types::Metadata;

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn via_map_ffi(
    scheme: *const c_char,
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
    callback: Option<extern "C" fn(u32, *const c_char)>,
    result: *mut FFIResult<od::blocking::Operator>,
) {
    unsafe {
        let scheme = match CStr::from_ptr(scheme).to_str() {
            Ok(s) => s,
            Err(_) => {
                *result = FFIResult::err("Failed to convert scheme to string");
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

        if let Some(callback) = callback
            && let Err(e) = log::set_boxed_logger(Box::new(HsLogger { callback }))
                .map(|()| log::set_max_level(log::LevelFilter::Debug))
        {
            *result = FFIResult::err_with_source(
                "Failed to register logger",
                od::Error::new(od::ErrorKind::Unexpected, e.to_string().as_str()),
            );
            return;
        }

        let res = match od::Operator::via_iter(scheme, map) {
            Ok(mut operator) => {
                operator = operator.layer(RetryLayer::new());
                if callback.is_some() {
                    operator = operator.layer(LoggingLayer::default());
                }

                let handle = RUNTIME.handle();
                let _enter = handle.enter();
                let op = od::blocking::Operator::new(operator).unwrap();
                FFIResult::ok(op)
            }
            Err(e) => FFIResult::err_with_source("Failed to create Operator", e),
        };

        *result = res;
    }
}

/// # Safety
///
/// * `operator` is a valid pointer to a `blocking::Operator`.
///
/// # Panics
///
/// * If `operator` is not a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_operator(operator: *mut od::blocking::Operator) {
    unsafe {
        if !operator.is_null() {
            drop(Box::from_raw(operator));
        }
    }
}

/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_read(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    result: *mut FFIResult<ByteSlice>,
) {
    unsafe {
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
}

/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_write(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    bytes: *const c_char,
    len: usize,
    result: *mut FFIResult<()>,
) {
    unsafe {
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
}

/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_is_exist(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    result: *mut FFIResult<bool>,
) {
    unsafe {
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

        let res = match op.exists(path_str) {
            Ok(exist) => FFIResult::ok(exist),
            Err(e) => FFIResult::err_with_source("Failed to check if path exists", e),
        };

        *result = res;
    }
}

/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_create_dir(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    result: *mut FFIResult<()>,
) {
    unsafe {
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
}

/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path_from` is a valid pointer to a nul terminated string.
/// * `path_to` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_copy(
    op: *mut od::blocking::Operator,
    path_from: *const c_char,
    path_to: *const c_char,
    result: *mut FFIResult<()>,
) {
    unsafe {
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
            Ok(_) => FFIResult::ok(()),
            Err(e) => FFIResult::err_with_source("Failed to copy", e),
        };

        *result = res;
    }
}

/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path_from` is a valid pointer to a nul terminated string.
/// * `path_to` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_rename(
    op: *mut od::blocking::Operator,
    path_from: *const c_char,
    path_to: *const c_char,
    result: *mut FFIResult<()>,
) {
    unsafe {
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
}

/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_delete(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    result: *mut FFIResult<()>,
) {
    unsafe {
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
}

/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_stat(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    result: *mut FFIResult<Metadata>,
) {
    unsafe {
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
}

/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_list(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    result: *mut FFIResult<*mut blocking::Lister>,
) {
    unsafe {
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
}

/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_scan(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    result: *mut FFIResult<*mut blocking::Lister>,
) {
    unsafe {
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

        let res = match op.lister_options(
            path_str,
            options::ListOptions {
                recursive: true,
                ..Default::default()
            },
        ) {
            Ok(lister) => FFIResult::ok(Box::into_raw(Box::new(lister))),
            Err(e) => FFIResult::err_with_source("Failed to scan", e),
        };

        *result = res;
    }
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lister_next(
    lister: *mut blocking::Lister,
    result: *mut FFIResult<*const c_char>,
) {
    unsafe {
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
}

/// # Safety
///
/// * `lister` is a valid pointer to a `BlockingLister`.
///
/// # Panics
///
/// * If `lister` is not a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_lister(lister: *mut blocking::Lister) {
    unsafe {
        if !lister.is_null() {
            drop(Box::from_raw(lister));
        }
    }
}

/// Get operator info (scheme)
///
/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_info(
    op: *mut od::blocking::Operator,
    result: *mut FFIResult<*const c_char>,
) {
    unsafe {
        let op = if op.is_null() {
            *result = FFIResult::err("Operator is null");
            return;
        } else {
            &mut *op
        };

        let info = op.info();
        let scheme_str = info.scheme().to_string();

        let res = match CString::new(scheme_str) {
            Ok(c_string) => {
                let ptr = c_string.into_raw() as *const c_char;
                FFIResult::ok(ptr)
            }
            Err(_) => FFIResult::err("Failed to convert scheme to C string"),
        };

        *result = res;
    }
}

/// Remove all files and directories recursively
///
/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_remove_all(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    result: *mut FFIResult<()>,
) {
    unsafe {
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

        let res = {
            use od::options::DeleteOptions;
            match op.delete_options(
                path_str,
                DeleteOptions {
                    recursive: true,
                    ..Default::default()
                },
            ) {
                Ok(()) => FFIResult::ok(()),
                Err(e) => FFIResult::err_with_source("Failed to remove all", e),
            }
        };

        *result = res;
    }
}

/// Creates a blocking writer for the given path
///
/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_writer(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    result: *mut FFIResult<*mut blocking::Writer>,
) {
    unsafe {
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

        let res = match op.writer(path_str) {
            Ok(writer) => FFIResult::ok(Box::into_raw(Box::new(writer))),
            Err(e) => FFIResult::err_with_source("Failed to create writer", e),
        };

        *result = res;
    }
}

/// Creates a blocking writer for the given path with append mode
///
/// # Safety
///
/// * `op` is a valid pointer to a `blocking::Operator`.
/// * `path` is a valid pointer to a nul terminated string.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `op` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn blocking_writer_append(
    op: *mut od::blocking::Operator,
    path: *const c_char,
    result: *mut FFIResult<*mut blocking::Writer>,
) {
    unsafe {
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

        // Create writer with append option
        let opts = od::options::WriteOptions {
            append: true,
            ..Default::default()
        };

        let res = match op.writer_options(path_str, opts) {
            Ok(writer) => FFIResult::ok(Box::into_raw(Box::new(writer))),
            Err(e) => FFIResult::err_with_source("Failed to create append writer", e),
        };

        *result = res;
    }
}

/// Write data using a blocking writer
///
/// # Safety
///
/// * `writer` is a valid pointer to a `blocking::Writer`.
/// * `bytes` is a valid pointer to a byte array.
/// * `len` is the length of `bytes`.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `writer` is not a valid pointer.
/// * If `bytes` is not a valid pointer, or `len` is more than the length of `bytes`.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn writer_write(
    writer: *mut blocking::Writer,
    bytes: *const c_char,
    len: usize,
    result: *mut FFIResult<()>,
) {
    unsafe {
        let writer = if writer.is_null() {
            *result = FFIResult::err("Writer is null");
            return;
        } else {
            &mut *writer
        };

        let bytes = Vec::from_raw_parts(bytes as *mut u8, len, len);

        let res = match writer.write(bytes.clone()) {
            Ok(_) => FFIResult::ok(()),
            Err(e) => FFIResult::err_with_source("Failed to write with writer", e),
        };

        *result = res;

        // bytes memory is controlled by Haskell, we can't drop it here
        mem::forget(bytes);
    }
}

/// Close a blocking writer
///
/// # Safety
///
/// * `writer` is a valid pointer to a `blocking::Writer`.
/// * `result` is a valid pointer, and has available memory to write to
///
/// # Panics
///
/// * If `writer` is not a valid pointer.
/// * If `result` is not a valid pointer, or does not have available memory to write to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn writer_close(
    writer: *mut blocking::Writer,
    result: *mut FFIResult<Metadata>,
) {
    unsafe {
        let writer = if writer.is_null() {
            *result = FFIResult::err("Writer is null");
            return;
        } else {
            &mut *writer
        };

        let res = match writer.close() {
            Ok(meta) => FFIResult::ok(meta.into()),
            Err(e) => FFIResult::err_with_source("Failed to close writer", e),
        };

        *result = res;
    }
}

/// Free a blocking writer
///
/// # Safety
///
/// * `writer` is a valid pointer to a `blocking::Writer`.
///
/// # Panics
///
/// * If `writer` is not a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_writer(writer: *mut blocking::Writer) {
    unsafe {
        if !writer.is_null() {
            drop(Box::from_raw(writer));
        }
    }
}
