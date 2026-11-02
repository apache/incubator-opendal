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

use crate::{
    buffer::{OpendalReadBuffer, WriteBufferSlot, take_payload},
    entry::into_entry_list_ptr,
    error::OpenDALError,
    executor::executor_or_default,
    metadata::OpendalMetadata,
    operator_info::{OpendalOperatorInfo, into_operator_info},
    options::{
        parse_delete_options, parse_list_options, parse_read_options, parse_stat_options,
        parse_write_options,
    },
    presign::into_presigned_request_ptr,
    result::{
        OpendalBoolResult, OpendalEntryListResult, OpendalMetadataResult,
        OpendalOperatorInfoResult, OpendalOperatorResult, OpendalOptionsResult,
        OpendalPresignedRequestResult, OpendalReadResult, OpendalResult,
    },
    utils::{
        collect_options, release_c_string, require_callback, require_cstr, require_data_ptr,
        require_op_handle,
    },
    validators::prelude::{
        validate_concurrent_limit_options, validate_retry_options, validate_throttle_options,
        validate_timeout_options,
    },
};

use std::collections::HashMap;
use std::ffi::c_void;
use std::os::raw::c_char;
use std::sync::Arc;
use std::time::Duration;

use asyncband::mutex::Mutex;
use futures::StreamExt;

use crate::executor::Executor;

/// Box metadata into the payload of an `OpendalMetadataResult`.
///
/// The pointer is released by `opendal_metadata_result_release`.
fn into_metadata_ptr(metadata: opendal::Metadata) -> *mut OpendalMetadata {
    Box::into_raw(Box::new(OpendalMetadata::from_metadata(metadata)))
}

/// The operator handle handed to .NET: the opendal operator plus the executor
/// it was bound to at construction. Every operation runs on that executor, and
/// the owned `Arc` keeps the runtime alive for as long as the operator lives.
pub(crate) struct OperatorHandle {
    op: opendal::Operator,
    executor: Arc<Executor>,
}

impl OperatorHandle {
    /// Build a handle around a derived operator (layered or duplicated), keeping the same executor.
    fn with_operator(&self, op: opendal::Operator) -> OperatorHandle {
        OperatorHandle {
            op,
            executor: self.executor.clone(),
        }
    }

    /// Clone the inner opendal operator into an owned value, e.g. to move it into a spawned task.
    fn operator(&self) -> opendal::Operator {
        self.op.clone()
    }
}

impl std::ops::Deref for OperatorHandle {
    type Target = opendal::Operator;

    fn deref(&self) -> &opendal::Operator {
        &self.op
    }
}

/// Callback signatures for async completion.
///
/// The callbacks are provided by the .NET side and must remain valid until
/// invoked by Rust. `VoidCallback` reports success or failure only, while
/// `MetadataCallback` carries the metadata that write, copy, and stat return.
type VoidCallback = extern "C" fn(context: i64, result: OpendalResult);
type BoolCallback = extern "C" fn(context: i64, result: OpendalBoolResult);
type ReadCallback = extern "C" fn(context: i64, result: OpendalReadResult);
type MetadataCallback = extern "C" fn(context: i64, result: OpendalMetadataResult);
type ListCallback = extern "C" fn(context: i64, result: OpendalEntryListResult);
type PresignCallback = extern "C" fn(context: i64, result: OpendalPresignedRequestResult);

/// Collect key/value arrays, parse them with `parse`, and box the result into
/// an options payload released by the matching `*_option_free`.
///
/// # Safety
///
/// Same contract as the exported `*_option_build` functions.
unsafe fn build_options<T>(
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
    parse: impl FnOnce(HashMap<String, String>) -> Result<T, OpenDALError>,
) -> OpendalOptionsResult {
    match unsafe { collect_options(keys, values, len) }.and_then(parse) {
        Ok(options) => OpendalOptionsResult::ok(Box::into_raw(Box::new(options)) as *mut c_void),
        Err(error) => OpendalOptionsResult::from_error(error),
    }
}

/// Release an options payload produced by `build_options`.
///
/// # Safety
///
/// - `options` must be null or a pointer returned by `build_options` for the
///   same `T`.
/// - This function must be called at most once for the same pointer.
unsafe fn free_options<T>(options: *mut T) {
    if options.is_null() {
        return;
    }

    drop(unsafe { Box::from_raw(options) });
}

/// Build constructor options from raw C string key/value arrays.
///
/// On success, the returned pointer must be released by
/// `constructor_option_free`.
/// # Safety
///
/// - When `len > 0`, `keys` and `values` must be non-null pointers to arrays
///   containing at least `len` C-string pointers.
/// - Each key/value entry must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn constructor_option_build(
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
) -> OpendalOptionsResult {
    unsafe { build_options(keys, values, len, Ok) }
}

/// # Safety
///
/// - `options` must be null or a pointer returned by
///   `constructor_option_build`.
/// - This function must be called at most once for the same pointer.
#[unsafe(no_mangle)]
pub extern "C" fn constructor_option_free(options: *mut HashMap<String, String>) {
    unsafe { free_options(options) }
}

/// Build read options from raw C string key/value arrays.
///
/// On success, the returned pointer must be released by `read_option_free`.
/// # Safety
///
/// - When `len > 0`, `keys` and `values` must be non-null pointers to arrays
///   containing at least `len` C-string pointers.
/// - Each key/value entry must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_option_build(
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
) -> OpendalOptionsResult {
    unsafe { build_options(keys, values, len, |values| parse_read_options(&values)) }
}

/// # Safety
///
/// - `options` must be null or a pointer returned by `read_option_build`.
/// - This function must be called at most once for the same pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_option_free(options: *mut opendal::options::ReadOptions) {
    unsafe { free_options(options) }
}

/// Build write options from raw C string key/value arrays.
///
/// On success, the returned pointer must be released by `write_option_free`.
/// # Safety
///
/// - When `len > 0`, `keys` and `values` must be non-null pointers to arrays
///   containing at least `len` C-string pointers.
/// - Each key/value entry must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn write_option_build(
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
) -> OpendalOptionsResult {
    unsafe { build_options(keys, values, len, |values| parse_write_options(&values)) }
}

/// # Safety
///
/// - `options` must be null or a pointer returned by `write_option_build`.
/// - This function must be called at most once for the same pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn write_option_free(options: *mut opendal::options::WriteOptions) {
    unsafe { free_options(options) }
}

/// Build stat options from raw C string key/value arrays.
///
/// On success, the returned pointer must be released by `stat_option_free`.
/// # Safety
///
/// - When `len > 0`, `keys` and `values` must be non-null pointers to arrays
///   containing at least `len` C-string pointers.
/// - Each key/value entry must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stat_option_build(
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
) -> OpendalOptionsResult {
    unsafe { build_options(keys, values, len, |values| parse_stat_options(&values)) }
}

/// # Safety
///
/// - `options` must be null or a pointer returned by `stat_option_build`.
/// - This function must be called at most once for the same pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stat_option_free(options: *mut opendal::options::StatOptions) {
    unsafe { free_options(options) }
}

/// Build list options from raw C string key/value arrays.
///
/// On success, the returned pointer must be released by `list_option_free`.
/// # Safety
///
/// - When `len > 0`, `keys` and `values` must be non-null pointers to arrays
///   containing at least `len` C-string pointers.
/// - Each key/value entry must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn list_option_build(
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
) -> OpendalOptionsResult {
    unsafe { build_options(keys, values, len, |values| parse_list_options(&values)) }
}

/// # Safety
///
/// - `options` must be null or a pointer returned by `list_option_build`.
/// - This function must be called at most once for the same pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn list_option_free(options: *mut opendal::options::ListOptions) {
    unsafe { free_options(options) }
}

/// Build delete options from raw C string key/value arrays.
///
/// On success, the returned pointer must be released by `delete_option_free`.
/// # Safety
///
/// - When `len > 0`, `keys` and `values` must be non-null pointers to arrays
///   containing at least `len` C-string pointers.
/// - Each key/value entry must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn delete_option_build(
    keys: *const *const c_char,
    values: *const *const c_char,
    len: usize,
) -> OpendalOptionsResult {
    unsafe { build_options(keys, values, len, |values| parse_delete_options(&values)) }
}

/// # Safety
///
/// - `options` must be null or a pointer returned by `delete_option_build`.
/// - This function must be called at most once for the same pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn delete_option_free(options: *mut opendal::options::DeleteOptions) {
    unsafe { free_options(options) }
}

/// Construct an OpenDAL operator instance from a scheme and key/value options.
///
/// Returns a pointer that must be released with `operator_free`.
/// # Safety
///
/// - `scheme` must be a valid null-terminated UTF-8 string.
/// - When `len > 0`, `keys` and `values` must be non-null and point to arrays
///   of at least `len` entries.
/// - Every key/value entry in those arrays must be a valid null-terminated
///   UTF-8 string.
/// - `executor` must be either null or a live pointer returned by
///   `executor_create`. The operator binds it for its whole lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_construct(
    scheme: *const c_char,
    options: *const HashMap<String, String>,
    executor: *const c_void,
) -> OpendalOperatorResult {
    match operator_construct_inner(scheme, options, executor) {
        Ok(op) => OpendalOperatorResult::ok(op),
        Err(error) => OpendalOperatorResult::from_error(error),
    }
}

fn operator_construct_inner(
    scheme: *const c_char,
    options: *const HashMap<String, String>,
    executor: *const c_void,
) -> Result<*mut c_void, OpenDALError> {
    let scheme = require_cstr(scheme, "scheme")?;
    let options = if options.is_null() {
        HashMap::default()
    } else {
        unsafe { (&*options).clone() }
    };
    // SAFETY: the caller keeps the executor handle alive across this call per
    // `operator_construct`'s contract; the clone owns the runtime afterwards.
    let executor = unsafe { executor_or_default(executor)? };
    let op =
        opendal::Operator::via_iter(scheme, options).map_err(OpenDALError::from_opendal_error)?;
    Ok(Box::into_raw(Box::new(OperatorHandle { op, executor })) as *mut c_void)
}

/// # Safety
///
/// - `op_handle` must be either null or a pointer returned by `operator_construct`.
/// - The pointer must not be used after this call.
/// - This function must be called at most once for the same pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_free(op_handle: *mut OperatorHandle) {
    if op_handle.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(op_handle));
    }
}

/// Get operator info payload.
///
/// On success, payload must be released by `opendal_operator_info_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_info_get(
    op_handle: *const OperatorHandle,
) -> OpendalOperatorInfoResult {
    match operator_info_get_inner(op_handle) {
        Ok(value) => OpendalOperatorInfoResult::ok(value),
        Err(error) => OpendalOperatorInfoResult::from_error(error),
    }
}

fn operator_info_get_inner(op_handle: *const OperatorHandle) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let info = into_operator_info(handle.info());
    Ok(Box::into_raw(Box::new(info)) as *mut c_void)
}

/// # Safety
///
/// - `info` must be either null or a pointer returned by `operator_info_get`.
/// - The pointer must not be used after this call.
/// - This function must be called at most once for the same pointer.
pub(crate) unsafe fn operator_info_free(info: *mut OpendalOperatorInfo) {
    if info.is_null() {
        return;
    }

    unsafe {
        let mut info = Box::from_raw(info);
        release_c_string(&mut info.scheme);
        release_c_string(&mut info.root);
        release_c_string(&mut info.name);
    }
}

/// Create a new operator layered with retry behavior.
///
/// The current operator is not modified. Returned pointer must be released with
/// `operator_free`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_layer_retry(
    op_handle: *const OperatorHandle,
    jitter: bool,
    factor: f32,
    min_delay_nanos: u64,
    max_delay_nanos: u64,
    max_times: usize,
) -> OpendalOperatorResult {
    match operator_layer_retry_inner(
        op_handle,
        jitter,
        factor,
        min_delay_nanos,
        max_delay_nanos,
        max_times,
    ) {
        Ok(value) => OpendalOperatorResult::ok(value),
        Err(error) => OpendalOperatorResult::from_error(error),
    }
}

fn operator_layer_retry_inner(
    op_handle: *const OperatorHandle,
    jitter: bool,
    factor: f32,
    min_delay_nanos: u64,
    max_delay_nanos: u64,
    max_times: usize,
) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    validate_retry_options(factor, min_delay_nanos, max_delay_nanos)?;

    let mut retry = opendal::layers::RetryLayer::new();
    retry = retry.with_factor(factor);
    retry = retry.with_min_delay(Duration::from_nanos(min_delay_nanos));
    retry = retry.with_max_delay(Duration::from_nanos(max_delay_nanos));
    retry = retry.with_max_times(max_times);
    if jitter {
        retry = retry.with_jitter();
    }

    Ok(Box::into_raw(Box::new(
        handle.with_operator(handle.operator().layer(retry)),
    )) as *mut c_void)
}

/// Create a new operator layered with concurrent-limit behavior.
///
/// The current operator is not modified. Returned pointer must be released with
/// `operator_free`.
///
/// `http_permits` limits concurrent HTTP requests independently of `permits`,
/// which limits concurrent operations. It is read only when `has_http_permits`
/// is true; otherwise the HTTP limit is left unset.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_layer_concurrent_limit(
    op_handle: *const OperatorHandle,
    permits: usize,
    http_permits: usize,
    has_http_permits: bool,
) -> OpendalOperatorResult {
    let http_permits = has_http_permits.then_some(http_permits);
    match operator_layer_concurrent_limit_inner(op_handle, permits, http_permits) {
        Ok(value) => OpendalOperatorResult::ok(value),
        Err(error) => OpendalOperatorResult::from_error(error),
    }
}

fn operator_layer_concurrent_limit_inner(
    op_handle: *const OperatorHandle,
    permits: usize,
    http_permits: Option<usize>,
) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    validate_concurrent_limit_options(permits, http_permits)?;

    let mut concurrent_limit = opendal::layers::ConcurrentLimitLayer::new(permits);
    if let Some(http_permits) = http_permits {
        concurrent_limit = concurrent_limit.with_http_concurrent_limit(http_permits);
    }

    Ok(Box::into_raw(Box::new(
        handle.with_operator(handle.operator().layer(concurrent_limit)),
    )) as *mut c_void)
}

/// Create a new operator layered with capability override behavior.
///
/// The current operator is not modified. Returned pointer must be released with
/// `operator_free`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `overrides` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub extern "C" fn operator_layer_capability_override(
    op_handle: *const OperatorHandle,
    overrides: *const c_char,
) -> OpendalOperatorResult {
    match operator_layer_capability_override_inner(op_handle, overrides) {
        Ok(value) => OpendalOperatorResult::ok(value),
        Err(error) => OpendalOperatorResult::from_error(error),
    }
}

fn operator_layer_capability_override_inner(
    op_handle: *const OperatorHandle,
    overrides: *const c_char,
) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let overrides = require_cstr(overrides, "capability overrides")?;
    let layer = opendal::layers::CapabilityOverrideLayer::from_overrides(overrides)
        .map_err(OpenDALError::from_opendal_error)?;

    Ok(Box::into_raw(Box::new(
        handle.with_operator(handle.operator().layer(layer)),
    )) as *mut c_void)
}

/// Create a new operator layered with timeout behavior.
///
/// The current operator is not modified. Returned pointer must be released with
/// `operator_free`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_layer_timeout(
    op_handle: *const OperatorHandle,
    timeout_nanos: u64,
    io_timeout_nanos: u64,
) -> OpendalOperatorResult {
    match operator_layer_timeout_inner(op_handle, timeout_nanos, io_timeout_nanos) {
        Ok(value) => OpendalOperatorResult::ok(value),
        Err(error) => OpendalOperatorResult::from_error(error),
    }
}

fn operator_layer_timeout_inner(
    op_handle: *const OperatorHandle,
    timeout_nanos: u64,
    io_timeout_nanos: u64,
) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    validate_timeout_options(timeout_nanos, io_timeout_nanos)?;

    let timeout = opendal::layers::TimeoutLayer::new()
        .with_timeout(Duration::from_nanos(timeout_nanos))
        .with_io_timeout(Duration::from_nanos(io_timeout_nanos));

    Ok(Box::into_raw(Box::new(
        handle.with_operator(handle.operator().layer(timeout)),
    )) as *mut c_void)
}

/// Create a new operator layered with throttle behavior.
///
/// The current operator is not modified. Returned pointer must be released with
/// `operator_free`.
///
/// `bandwidth` is the maximum number of bytes allowed through per second, and
/// `burst` is the maximum number of bytes allowed through at once. `burst` must
/// exceed the largest possible operation size, otherwise that operation can
/// never acquire enough quota to proceed.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_layer_throttle(
    op_handle: *const OperatorHandle,
    bandwidth: u32,
    burst: u32,
) -> OpendalOperatorResult {
    match operator_layer_throttle_inner(op_handle, bandwidth, burst) {
        Ok(value) => OpendalOperatorResult::ok(value),
        Err(error) => OpendalOperatorResult::from_error(error),
    }
}

fn operator_layer_throttle_inner(
    op_handle: *const OperatorHandle,
    bandwidth: u32,
    burst: u32,
) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    validate_throttle_options(bandwidth, burst)?;

    let throttle = opendal::layers::ThrottleLayer::new(bandwidth, burst);
    Ok(Box::into_raw(Box::new(
        handle.with_operator(handle.operator().layer(throttle)),
    )) as *mut c_void)
}

/// Create a new operator layered with MIME-guess behavior.
///
/// The current operator is not modified. Returned pointer must be released with
/// `operator_free`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_layer_mime_guess(
    op_handle: *const OperatorHandle,
) -> OpendalOperatorResult {
    match operator_layer_mime_guess_inner(op_handle) {
        Ok(value) => OpendalOperatorResult::ok(value),
        Err(error) => OpendalOperatorResult::from_error(error),
    }
}

fn operator_layer_mime_guess_inner(
    op_handle: *const OperatorHandle,
) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;

    let mime_guess = opendal::layers::MimeGuessLayer::default();
    Ok(Box::into_raw(Box::new(
        handle.with_operator(handle.operator().layer(mime_guess)),
    )) as *mut c_void)
}

/// Duplicate an operator instance.
///
/// Returned pointer must be released with `operator_free`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_duplicate(op_handle: *const OperatorHandle) -> OpendalOperatorResult {
    match operator_duplicate_inner(op_handle) {
        Ok(value) => OpendalOperatorResult::ok(value),
        Err(error) => OpendalOperatorResult::from_error(error),
    }
}

fn operator_duplicate_inner(op_handle: *const OperatorHandle) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    Ok(Box::into_raw(Box::new(handle.with_operator(handle.operator()))) as *mut c_void)
}

/// Delete `path` synchronously with options.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - `options` must be null or a pointer returned by `delete_option_build`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_delete_with_options(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::DeleteOptions,
) -> OpendalResult {
    match operator_delete_with_options_inner(op_handle, path, options) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_delete_with_options_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::DeleteOptions,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?;

    let options = if options.is_null() {
        opendal::options::DeleteOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    executor
        .block_on(handle.delete_options(path, options))
        .map_err(OpenDALError::from_opendal_error)
}

/// Delete `path` asynchronously with options.
///
/// The callback is invoked exactly once with the final result.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - `options` must be null or a pointer returned by `delete_option_build`.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_delete_with_options_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::DeleteOptions,
    callback: Option<VoidCallback>,
    context: i64,
) -> OpendalResult {
    match operator_delete_with_options_async_inner(op_handle, path, options, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_delete_with_options_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::DeleteOptions,
    callback: Option<VoidCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;

    let options = if options.is_null() {
        opendal::options::DeleteOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .delete_options(&path, options)
            .await
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(()) => OpendalResult::ok(),
                Err(error) => OpendalResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Create directory at `path` synchronously.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub extern "C" fn operator_create_dir(
    op_handle: *const OperatorHandle,
    path: *const c_char,
) -> OpendalResult {
    match operator_create_dir_inner(op_handle, path) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_create_dir_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?;

    executor
        .block_on(handle.create_dir(path))
        .map_err(OpenDALError::from_opendal_error)
}

/// Create directory at `path` asynchronously.
///
/// The callback is invoked exactly once with the final result.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_create_dir_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    callback: Option<VoidCallback>,
    context: i64,
) -> OpendalResult {
    match operator_create_dir_async_inner(op_handle, path, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_create_dir_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    callback: Option<VoidCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .create_dir(&path)
            .await
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(()) => OpendalResult::ok(),
                Err(error) => OpendalResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Copy from `source_path` to `target_path` synchronously.
///
/// On success, the returned payload must be released with
/// `opendal_metadata_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `source_path` and `target_path` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub extern "C" fn operator_copy(
    op_handle: *const OperatorHandle,
    source_path: *const c_char,
    target_path: *const c_char,
) -> OpendalMetadataResult {
    match operator_copy_inner(op_handle, source_path, target_path) {
        Ok(value) => OpendalMetadataResult::ok(value as *mut c_void),
        Err(error) => OpendalMetadataResult::from_error(error),
    }
}

fn operator_copy_inner(
    op_handle: *const OperatorHandle,
    source_path: *const c_char,
    target_path: *const c_char,
) -> Result<*mut OpendalMetadata, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let source_path = require_cstr(source_path, "source_path")?;
    let target_path = require_cstr(target_path, "target_path")?;

    let metadata = executor
        .block_on(handle.copy(source_path, target_path))
        .map_err(OpenDALError::from_opendal_error)?;
    Ok(into_metadata_ptr(metadata))
}

/// Copy from `source_path` to `target_path` asynchronously.
///
/// The callback is invoked exactly once. On success, the callback result must
/// be released with `opendal_metadata_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `source_path` and `target_path` must be valid null-terminated UTF-8 strings.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_copy_async(
    op_handle: *const OperatorHandle,
    source_path: *const c_char,
    target_path: *const c_char,
    callback: Option<MetadataCallback>,
    context: i64,
) -> OpendalResult {
    match operator_copy_async_inner(op_handle, source_path, target_path, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_copy_async_inner(
    op_handle: *const OperatorHandle,
    source_path: *const c_char,
    target_path: *const c_char,
    callback: Option<MetadataCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let source_path = require_cstr(source_path, "source_path")?.to_string();
    let target_path = require_cstr(target_path, "target_path")?.to_string();
    let callback = require_callback(callback)?;

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .copy(&source_path, &target_path)
            .await
            .map(into_metadata_ptr)
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(value) => OpendalMetadataResult::ok(value as *mut c_void),
                Err(error) => OpendalMetadataResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Rename from `source_path` to `target_path` synchronously.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `source_path` and `target_path` must be valid null-terminated UTF-8 strings.
#[unsafe(no_mangle)]
pub extern "C" fn operator_rename(
    op_handle: *const OperatorHandle,
    source_path: *const c_char,
    target_path: *const c_char,
) -> OpendalResult {
    match operator_rename_inner(op_handle, source_path, target_path) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_rename_inner(
    op_handle: *const OperatorHandle,
    source_path: *const c_char,
    target_path: *const c_char,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let source_path = require_cstr(source_path, "source_path")?;
    let target_path = require_cstr(target_path, "target_path")?;

    executor
        .block_on(handle.rename(source_path, target_path))
        .map_err(OpenDALError::from_opendal_error)
}

/// Rename from `source_path` to `target_path` asynchronously.
///
/// The callback is invoked exactly once with the final result.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `source_path` and `target_path` must be valid null-terminated UTF-8 strings.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_rename_async(
    op_handle: *const OperatorHandle,
    source_path: *const c_char,
    target_path: *const c_char,
    callback: Option<VoidCallback>,
    context: i64,
) -> OpendalResult {
    match operator_rename_async_inner(op_handle, source_path, target_path, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_rename_async_inner(
    op_handle: *const OperatorHandle,
    source_path: *const c_char,
    target_path: *const c_char,
    callback: Option<VoidCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let source_path = require_cstr(source_path, "source_path")?.to_string();
    let target_path = require_cstr(target_path, "target_path")?.to_string();
    let callback = require_callback(callback)?;

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .rename(&source_path, &target_path)
            .await
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(()) => OpendalResult::ok(),
                Err(error) => OpendalResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Presign read asynchronously.
///
/// The callback is invoked exactly once with the final result.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_presign_read_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    expire_nanos: u64,
    callback: Option<PresignCallback>,
    context: i64,
) -> OpendalResult {
    match operator_presign_read_async_inner(op_handle, path, expire_nanos, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_presign_read_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    expire_nanos: u64,
    callback: Option<PresignCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;
    let expire = Duration::from_nanos(expire_nanos);

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .presign_read(&path, expire)
            .await
            .map_err(OpenDALError::from_opendal_error)
            .and_then(into_presigned_request_ptr);

        callback(
            context,
            match result {
                Ok(value) => OpendalPresignedRequestResult::ok(value),
                Err(error) => OpendalPresignedRequestResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Presign write asynchronously.
///
/// The callback is invoked exactly once with the final result.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_presign_write_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    expire_nanos: u64,
    callback: Option<PresignCallback>,
    context: i64,
) -> OpendalResult {
    match operator_presign_write_async_inner(op_handle, path, expire_nanos, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_presign_write_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    expire_nanos: u64,
    callback: Option<PresignCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;
    let expire = Duration::from_nanos(expire_nanos);

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .presign_write(&path, expire)
            .await
            .map_err(OpenDALError::from_opendal_error)
            .and_then(into_presigned_request_ptr);

        callback(
            context,
            match result {
                Ok(value) => OpendalPresignedRequestResult::ok(value),
                Err(error) => OpendalPresignedRequestResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Presign stat asynchronously.
///
/// The callback is invoked exactly once with the final result.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_presign_stat_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    expire_nanos: u64,
    callback: Option<PresignCallback>,
    context: i64,
) -> OpendalResult {
    match operator_presign_stat_async_inner(op_handle, path, expire_nanos, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_presign_stat_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    expire_nanos: u64,
    callback: Option<PresignCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;
    let expire = Duration::from_nanos(expire_nanos);

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .presign_stat(&path, expire)
            .await
            .map_err(OpenDALError::from_opendal_error)
            .and_then(into_presigned_request_ptr);

        callback(
            context,
            match result {
                Ok(value) => OpendalPresignedRequestResult::ok(value),
                Err(error) => OpendalPresignedRequestResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Presign delete asynchronously.
///
/// The callback is invoked exactly once with the final result.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_presign_delete_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    expire_nanos: u64,
    callback: Option<PresignCallback>,
    context: i64,
) -> OpendalResult {
    match operator_presign_delete_async_inner(op_handle, path, expire_nanos, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_presign_delete_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    expire_nanos: u64,
    callback: Option<PresignCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;
    let expire = Duration::from_nanos(expire_nanos);

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .presign_delete(&path, expire)
            .await
            .map_err(OpenDALError::from_opendal_error)
            .and_then(into_presigned_request_ptr);

        callback(
            context,
            match result {
                Ok(value) => OpendalPresignedRequestResult::ok(value),
                Err(error) => OpendalPresignedRequestResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Create an input stream for `path` with read options.
///
/// Returned pointer must be released by `operator_input_stream_free`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub extern "C" fn operator_input_stream_create(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::ReadOptions,
) -> OpendalOperatorResult {
    match operator_input_stream_create_inner(op_handle, path, options) {
        Ok(value) => OpendalOperatorResult::ok(value),
        Err(error) => OpendalOperatorResult::from_error(error),
    }
}

/// Streaming reader backing `OperatorInputStream`.
///
/// The underlying byte stream is async; the sync FFI path blocks on it
/// through the owning executor while the async path spawns onto it. The
/// mutex is defensive — the C# stream contract already forbids concurrent
/// reads — and the `Arc` lets in-flight tasks outlive an early free safely.
struct InputStream {
    executor: Arc<Executor>,
    inner: Arc<Mutex<opendal::FuturesBytesStream>>,
}

fn require_input_stream<'a>(stream: *mut c_void) -> Result<&'a InputStream, OpenDALError> {
    if stream.is_null() {
        return Err(crate::utils::config_invalid_error(
            "input stream pointer is null",
        ));
    }

    Ok(unsafe { &*(stream as *const InputStream) })
}

/// Convert one polled chunk into an FFI read payload.
///
/// EOF becomes an empty buffer, matching the one-shot read contract.
fn next_chunk_to_buffer(
    value: Option<std::io::Result<bytes::Bytes>>,
) -> Result<OpendalReadBuffer, OpenDALError> {
    value
        .transpose()
        .map_err(|err| match err.downcast::<opendal::Error>() {
            Ok(error) => OpenDALError::from_opendal_error(error),
            Err(err) => OpenDALError::from_opendal_error(opendal::Error::new(
                opendal::ErrorKind::Unexpected,
                err.to_string(),
            )),
        })
        .map(|chunk| {
            chunk
                .map(|bytes| OpendalReadBuffer::from_buffer(bytes.into()))
                .unwrap_or_else(OpendalReadBuffer::empty)
        })
}

fn operator_input_stream_create_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::ReadOptions,
) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let options = if options.is_null() {
        opendal::options::ReadOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    let range = options.range;
    let reader_options = opendal::options::ReaderOptions {
        version: options.version,
        if_match: options.if_match,
        if_none_match: options.if_none_match,
        if_modified_since: options.if_modified_since,
        if_unmodified_since: options.if_unmodified_since,
        content_length_hint: options.content_length_hint,
        concurrent: options.concurrent,
        chunk: options.chunk,
        gap: options.gap,
        ..Default::default()
    };

    let stream_op = handle.operator();
    let stream = executor
        .block_on(async move {
            let reader = stream_op.reader_options(&path, reader_options).await?;
            reader.into_bytes_stream(range).await
        })
        .map_err(OpenDALError::from_opendal_error)?;

    Ok(Box::into_raw(Box::new(InputStream {
        executor,
        inner: Arc::new(Mutex::new(stream)),
    })) as *mut c_void)
}

/// Read next bytes chunk from input stream.
///
/// Returns an empty buffer when EOF is reached.
/// # Safety
///
/// - `stream` must be a valid pointer returned by `operator_input_stream_create`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_input_stream_read_next(stream: *mut c_void) -> OpendalReadResult {
    match operator_input_stream_read_next_inner(stream) {
        Ok(buffer) => OpendalReadResult::ok(buffer),
        Err(error) => OpendalReadResult::from_error(error),
    }
}

fn operator_input_stream_read_next_inner(
    stream: *mut c_void,
) -> Result<OpendalReadBuffer, OpenDALError> {
    let stream = require_input_stream(stream)?;
    let inner = stream.inner.clone();
    let value = stream
        .executor
        .block_on(async move { inner.lock().await.next().await });

    next_chunk_to_buffer(value)
}

/// Read next bytes chunk from input stream asynchronously.
///
/// The callback is invoked exactly once; EOF is reported as an empty buffer.
/// On successful reads, the callback result must be released with
/// `opendal_read_result_release`. The stream must not be read again, on
/// either path, until the callback fires.
/// # Safety
///
/// - `stream` must be a valid pointer returned by `operator_input_stream_create`.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_input_stream_read_next_async(
    stream: *mut c_void,
    callback: Option<ReadCallback>,
    context: i64,
) -> OpendalResult {
    match operator_input_stream_read_next_async_inner(stream, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_input_stream_read_next_async_inner(
    stream: *mut c_void,
    callback: Option<ReadCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let stream = require_input_stream(stream)?;
    let callback = require_callback(callback)?;

    let inner = stream.inner.clone();
    stream.executor.spawn(async move {
        let value = inner.lock().await.next().await;

        callback(
            context,
            match next_chunk_to_buffer(value) {
                Ok(buffer) => OpendalReadResult::ok(buffer),
                Err(error) => OpendalReadResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// # Safety
///
/// - `stream` must be null or a pointer returned by `operator_input_stream_create`.
/// - Must be called at most once for the same pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_input_stream_free(stream: *mut c_void) {
    if stream.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(stream as *mut InputStream));
    }
}

/// Create an output stream for `path` with write options.
///
/// Returned pointer must be released by `operator_output_stream_free`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub extern "C" fn operator_output_stream_create(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::WriteOptions,
) -> OpendalOperatorResult {
    match operator_output_stream_create_inner(op_handle, path, options) {
        Ok(value) => OpendalOperatorResult::ok(value),
        Err(error) => OpendalOperatorResult::from_error(error),
    }
}

/// Streaming writer backing `OperatorOutputStream`.
///
/// Same split as [`InputStream`]: async writer underneath, sync FFI blocks on
/// it, async FFI spawns onto it. The C# stream contract forbids concurrent
/// writes; the mutex is defensive and the `Arc` keeps in-flight tasks safe
/// against an early free.
struct OutputStream {
    executor: Arc<Executor>,
    inner: Arc<Mutex<opendal::Writer>>,
}

fn require_output_stream<'a>(stream: *mut c_void) -> Result<&'a OutputStream, OpenDALError> {
    if stream.is_null() {
        return Err(crate::utils::config_invalid_error(
            "output stream pointer is null",
        ));
    }

    Ok(unsafe { &*(stream as *const OutputStream) })
}

fn operator_output_stream_create_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::WriteOptions,
) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let options = if options.is_null() {
        opendal::options::WriteOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    let writer_op = handle.operator();
    let writer = executor
        .block_on(async move { writer_op.writer_options(&path, options).await })
        .map_err(OpenDALError::from_opendal_error)?;

    Ok(Box::into_raw(Box::new(OutputStream {
        executor,
        inner: Arc::new(Mutex::new(writer)),
    })) as *mut c_void)
}

/// Write bytes to output stream.
/// # Safety
///
/// - `stream` must be a valid pointer returned by `operator_output_stream_create`.
/// - When `len > 0`, `data` must be non-null and readable for `len` bytes.
#[unsafe(no_mangle)]
pub extern "C" fn operator_output_stream_write(
    stream: *mut c_void,
    data: *const u8,
    len: usize,
) -> OpendalResult {
    match operator_output_stream_write_inner(stream, data, len) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_output_stream_write_inner(
    stream: *mut c_void,
    data: *const u8,
    len: usize,
) -> Result<(), OpenDALError> {
    let stream = require_output_stream(stream)?;
    require_data_ptr(data, len)?;

    let payload = if len == 0 {
        bytes::Bytes::new()
    } else {
        bytes::Bytes::copy_from_slice(unsafe { std::slice::from_raw_parts(data, len) })
    };

    let inner = stream.inner.clone();
    stream
        .executor
        .block_on(async move { inner.lock().await.write(payload).await })
        .map(|_| ())
        .map_err(OpenDALError::from_opendal_error)
}

/// Write bytes to output stream asynchronously.
///
/// The payload is copied before this function returns, so the caller only has
/// to keep `data` alive for the duration of this call. The callback is
/// invoked exactly once. The stream must not be written to again, on either
/// path, until the callback fires.
/// # Safety
///
/// - `stream` must be a valid pointer returned by `operator_output_stream_create`.
/// - When `len > 0`, `data` must be non-null and readable for `len` bytes.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_output_stream_write_async(
    stream: *mut c_void,
    data: *const u8,
    len: usize,
    callback: Option<VoidCallback>,
    context: i64,
) -> OpendalResult {
    match operator_output_stream_write_async_inner(stream, data, len, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_output_stream_write_async_inner(
    stream: *mut c_void,
    data: *const u8,
    len: usize,
    callback: Option<VoidCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let stream = require_output_stream(stream)?;
    require_data_ptr(data, len)?;
    let callback = require_callback(callback)?;

    let payload = if len == 0 {
        bytes::Bytes::new()
    } else {
        bytes::Bytes::copy_from_slice(unsafe { std::slice::from_raw_parts(data, len) })
    };

    let inner = stream.inner.clone();
    stream.executor.spawn(async move {
        let result = inner
            .lock()
            .await
            .write(payload)
            .await
            .map(|_| ())
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(()) => OpendalResult::ok(),
                Err(error) => OpendalResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Flush output stream.
///
/// The underlying writer only persists data on close, so this is a no-op that
/// exists to keep the FFI surface symmetric.
/// # Safety
///
/// - `stream` must be a valid pointer returned by `operator_output_stream_create`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_output_stream_flush(stream: *mut c_void) -> OpendalResult {
    match operator_output_stream_flush_inner(stream) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_output_stream_flush_inner(stream: *mut c_void) -> Result<(), OpenDALError> {
    require_output_stream(stream)?;
    Ok(())
}

/// Close output stream.
/// # Safety
///
/// - `stream` must be a valid pointer returned by `operator_output_stream_create`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_output_stream_close(stream: *mut c_void) -> OpendalResult {
    match operator_output_stream_close_inner(stream) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_output_stream_close_inner(stream: *mut c_void) -> Result<(), OpenDALError> {
    let stream = require_output_stream(stream)?;
    let inner = stream.inner.clone();
    stream
        .executor
        .block_on(async move { inner.lock().await.close().await })
        .map(|_| ())
        .map_err(OpenDALError::from_opendal_error)
}

/// Close output stream asynchronously.
///
/// The callback is invoked exactly once. After a successful close the handle
/// only needs `operator_output_stream_free`.
/// # Safety
///
/// - `stream` must be a valid pointer returned by `operator_output_stream_create`.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_output_stream_close_async(
    stream: *mut c_void,
    callback: Option<VoidCallback>,
    context: i64,
) -> OpendalResult {
    match operator_output_stream_close_async_inner(stream, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_output_stream_close_async_inner(
    stream: *mut c_void,
    callback: Option<VoidCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let stream = require_output_stream(stream)?;
    let callback = require_callback(callback)?;

    let inner = stream.inner.clone();
    stream.executor.spawn(async move {
        let result = inner
            .lock()
            .await
            .close()
            .await
            .map(|_| ())
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(()) => OpendalResult::ok(),
                Err(error) => OpendalResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// # Safety
///
/// - `stream` must be null or a pointer returned by `operator_output_stream_create`.
/// - Must be called at most once for the same pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_output_stream_free(stream: *mut c_void) {
    if stream.is_null() {
        return;
    }

    unsafe {
        drop(Box::from_raw(stream as *mut OutputStream));
    }
}

/// Write the committed bytes of a write buffer to `path` synchronously.
///
/// On a non-error return the contents belong to the write and the caller
/// must not write through the buffer's pointers again; the handle itself
/// still needs `write_buffer_free`. An error raised before the payload is
/// taken leaves the slot untouched, while a backend failure after it has
/// already consumed the contents.
///
/// On success, the returned payload must be released with
/// `opendal_metadata_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - `buffer` must be a handle from `write_buffer_create` whose contents
///   have not been consumed, and must not have been freed.
/// - Calls taking the same buffer handle must not run concurrently.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_write_with_options(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    buffer: *mut c_void,
    committed_in_current: usize,
    options: *const opendal::options::WriteOptions,
) -> OpendalMetadataResult {
    match operator_write_with_options_inner(op_handle, path, buffer, committed_in_current, options)
    {
        Ok(value) => OpendalMetadataResult::ok(value as *mut c_void),
        Err(error) => OpendalMetadataResult::from_error(error),
    }
}

fn operator_write_with_options_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    buffer: *mut c_void,
    committed_in_current: usize,
    options: *const opendal::options::WriteOptions,
) -> Result<*mut OpendalMetadata, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?;
    if buffer.is_null() {
        return Err(crate::utils::config_invalid_error(
            "write buffer handle is null",
        ));
    }
    let options = if options.is_null() {
        opendal::options::WriteOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    // Taken only after every fallible check above, so an error result means
    // ownership never transferred and the buffer stays usable.
    let slot = unsafe { &mut *(buffer as *mut WriteBufferSlot) };
    let payload = take_payload(slot, committed_in_current)?;

    let metadata = executor
        .block_on(handle.write_options(path, payload, options))
        .map_err(OpenDALError::from_opendal_error)?;
    Ok(into_metadata_ptr(metadata))
}

/// Write the committed bytes of a write buffer to `path` asynchronously.
///
/// The callback is invoked exactly once. Ownership behaves as in the sync
/// variant: consumed on a non-error return, untouched on error. On success,
/// the callback result must be released with
/// `opendal_metadata_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - `buffer` must be a handle from `write_buffer_create` whose contents
///   have not been consumed, and must not have been freed.
/// - Calls taking the same buffer handle must not run concurrently.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_write_with_options_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    buffer: *mut c_void,
    committed_in_current: usize,
    options: *const opendal::options::WriteOptions,
    callback: Option<MetadataCallback>,
    context: i64,
) -> OpendalResult {
    match operator_write_with_options_async_inner(
        op_handle,
        path,
        buffer,
        committed_in_current,
        options,
        callback,
        context,
    ) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

// Mirrors the `extern "C"` signature above, which clippy exempts because a C ABI
// is not a design choice. This helper exists only to give that function a body it
// can write with `?`.
#[allow(clippy::too_many_arguments)]
fn operator_write_with_options_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    buffer: *mut c_void,
    committed_in_current: usize,
    options: *const opendal::options::WriteOptions,
    callback: Option<MetadataCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;
    if buffer.is_null() {
        return Err(crate::utils::config_invalid_error(
            "write buffer handle is null",
        ));
    }
    let options = if options.is_null() {
        opendal::options::WriteOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    // Taken only after every fallible check above, so an error result means
    // ownership never transferred and the buffer stays usable.
    let slot = unsafe { &mut *(buffer as *mut WriteBufferSlot) };
    let payload = take_payload(slot, committed_in_current)?;

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .write_options(&path, payload, options)
            .await
            .map(into_metadata_ptr)
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(value) => OpendalMetadataResult::ok(value as *mut c_void),
                Err(error) => OpendalMetadataResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Write a caller-owned byte range to `path` synchronously.
///
/// The bytes are copied before the write, so `data` only has to stay valid
/// for the duration of this call. On success, the returned payload must be
/// released with `opendal_metadata_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - When `len > 0`, `data` must be non-null and readable for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_write_bytes_with_options(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    data: *const u8,
    len: usize,
    options: *const opendal::options::WriteOptions,
) -> OpendalMetadataResult {
    match operator_write_bytes_with_options_inner(op_handle, path, data, len, options) {
        Ok(value) => OpendalMetadataResult::ok(value as *mut c_void),
        Err(error) => OpendalMetadataResult::from_error(error),
    }
}

fn operator_write_bytes_with_options_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    data: *const u8,
    len: usize,
    options: *const opendal::options::WriteOptions,
) -> Result<*mut OpendalMetadata, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?;
    require_data_ptr(data, len)?;
    let options = if options.is_null() {
        opendal::options::WriteOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    // Copied explicitly: the only `Into<Buffer>` impl for slices is
    // `From<&'static [u8]>`, which is zero-copy, so passing the raw slice
    // would launder the caller's pointer into `'static` and let backends
    // retain memory that is only pinned for the duration of this call.
    let payload = if len == 0 {
        bytes::Bytes::new()
    } else {
        bytes::Bytes::copy_from_slice(unsafe { std::slice::from_raw_parts(data, len) })
    };

    let metadata = executor
        .block_on(handle.write_options(path, payload, options))
        .map_err(OpenDALError::from_opendal_error)?;
    Ok(into_metadata_ptr(metadata))
}

/// Write a caller-owned byte range to `path` asynchronously.
///
/// The bytes are copied before the task is spawned, so `data` only has to
/// stay valid for the duration of this call. The callback is invoked exactly
/// once. On success, the callback result must be released with
/// `opendal_metadata_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - When `len > 0`, `data` must be non-null and readable for `len` bytes.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_write_bytes_with_options_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    data: *const u8,
    len: usize,
    options: *const opendal::options::WriteOptions,
    callback: Option<MetadataCallback>,
    context: i64,
) -> OpendalResult {
    match operator_write_bytes_with_options_async_inner(
        op_handle, path, data, len, options, callback, context,
    ) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

// Mirrors the `extern "C"` signature above, which clippy exempts because a C ABI
// is not a design choice. This helper exists only to give that function a body it
// can write with `?`.
#[allow(clippy::too_many_arguments)]
fn operator_write_bytes_with_options_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    data: *const u8,
    len: usize,
    options: *const opendal::options::WriteOptions,
    callback: Option<MetadataCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    require_data_ptr(data, len)?;
    let callback = require_callback(callback)?;
    let options = if options.is_null() {
        opendal::options::WriteOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    // Copied before the task is spawned, so the caller only has to keep its
    // array alive for the duration of this call.
    let payload = if len == 0 {
        bytes::Bytes::new()
    } else {
        bytes::Bytes::copy_from_slice(unsafe { std::slice::from_raw_parts(data, len) })
    };

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .write_options(&path, payload, options)
            .await
            .map(into_metadata_ptr)
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(value) => OpendalMetadataResult::ok(value as *mut c_void),
                Err(error) => OpendalMetadataResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Read bytes from `path` synchronously with options.
///
/// On success, the returned buffer must be released with `opendal_read_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - When `option_len > 0`, `option_keys` and `option_values` must be valid arrays.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn operator_read_with_options(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::ReadOptions,
) -> OpendalReadResult {
    match operator_read_with_options_inner(op_handle, path, options) {
        Ok(value) => OpendalReadResult::ok(value),
        Err(error) => OpendalReadResult::from_error(error),
    }
}

fn operator_read_with_options_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::ReadOptions,
) -> Result<OpendalReadBuffer, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?;
    let options = if options.is_null() {
        opendal::options::ReadOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    let value = executor
        .block_on(handle.read_options(path, options))
        .map_err(OpenDALError::from_opendal_error)?;

    Ok(OpendalReadBuffer::from_buffer(value))
}

/// Read bytes from `path` asynchronously with options.
///
/// The callback is invoked exactly once. On successful reads, the callback
/// result must be released with `opendal_read_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - When `option_len > 0`, `option_keys` and `option_values` must be valid arrays.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_read_with_options_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::ReadOptions,
    callback: Option<ReadCallback>,
    context: i64,
) -> OpendalResult {
    match operator_read_with_options_async_inner(op_handle, path, options, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_read_with_options_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::ReadOptions,
    callback: Option<ReadCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;
    let options = if options.is_null() {
        opendal::options::ReadOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .read_options(&path, options)
            .await
            .map(OpendalReadBuffer::from_buffer)
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(value) => OpendalReadResult::ok(value),
                Err(error) => OpendalReadResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Stat `path` synchronously with options.
///
/// On success, returned payload must be released with `opendal_metadata_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - When `option_len > 0`, `option_keys` and `option_values` must be valid arrays.
#[unsafe(no_mangle)]
pub extern "C" fn operator_stat_with_options(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::StatOptions,
) -> OpendalMetadataResult {
    match operator_stat_with_options_inner(op_handle, path, options) {
        Ok(value) => OpendalMetadataResult::ok(value as *mut c_void),
        Err(error) => OpendalMetadataResult::from_error(error),
    }
}

fn operator_stat_with_options_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::StatOptions,
) -> Result<*mut OpendalMetadata, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?;
    let options = if options.is_null() {
        opendal::options::StatOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    let metadata = executor
        .block_on(handle.stat_options(path, options))
        .map_err(OpenDALError::from_opendal_error)?;
    Ok(into_metadata_ptr(metadata))
}

/// Stat `path` asynchronously with options.
///
/// The callback is invoked exactly once. On success, callback result must be
/// released with `opendal_metadata_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - When `option_len > 0`, `option_keys` and `option_values` must be valid arrays.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_stat_with_options_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::StatOptions,
    callback: Option<MetadataCallback>,
    context: i64,
) -> OpendalResult {
    match operator_stat_with_options_async_inner(op_handle, path, options, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_stat_with_options_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::StatOptions,
    callback: Option<MetadataCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;
    let options = if options.is_null() {
        opendal::options::StatOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .stat_options(&path, options)
            .await
            .map(into_metadata_ptr)
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(value) => OpendalMetadataResult::ok(value as *mut c_void),
                Err(error) => OpendalMetadataResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Check whether `path` exists synchronously.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub extern "C" fn operator_exists(
    op_handle: *const OperatorHandle,
    path: *const c_char,
) -> OpendalBoolResult {
    match operator_exists_inner(op_handle, path) {
        Ok(value) => OpendalBoolResult::ok(value as u8),
        Err(error) => OpendalBoolResult::from_error(error),
    }
}

fn operator_exists_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
) -> Result<bool, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?;

    executor
        .block_on(handle.exists(path))
        .map_err(OpenDALError::from_opendal_error)
}

/// Check whether `path` exists asynchronously.
///
/// The callback is invoked exactly once with the final result.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_exists_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    callback: Option<BoolCallback>,
    context: i64,
) -> OpendalResult {
    match operator_exists_async_inner(op_handle, path, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_exists_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    callback: Option<BoolCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .exists(&path)
            .await
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(value) => OpendalBoolResult::ok(value as u8),
                Err(error) => OpendalBoolResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// Check whether the operator can reach its service.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
#[unsafe(no_mangle)]
pub extern "C" fn operator_check(op_handle: *const OperatorHandle) -> OpendalResult {
    match operator_check_inner(op_handle) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_check_inner(op_handle: *const OperatorHandle) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();

    executor
        .block_on(handle.check())
        .map_err(OpenDALError::from_opendal_error)
}

/// Check whether the operator can reach its service asynchronously.
///
/// The callback is invoked exactly once with the final result.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_check_async(
    op_handle: *const OperatorHandle,
    callback: Option<VoidCallback>,
    context: i64,
) -> OpendalResult {
    match operator_check_async_inner(op_handle, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_check_async_inner(
    op_handle: *const OperatorHandle,
    callback: Option<VoidCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let callback = require_callback(callback)?;

    let op = handle.operator();
    executor.spawn(async move {
        let result = op.check().await.map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(()) => OpendalResult::ok(),
                Err(error) => OpendalResult::from_error(error),
            },
        );
    });

    Ok(())
}

/// List entries from `path` synchronously with options.
///
/// On success, returned payload must be released with `opendal_entry_list_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - When `option_len > 0`, `option_keys` and `option_values` must be valid arrays.
#[unsafe(no_mangle)]
pub extern "C" fn operator_list_with_options(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::ListOptions,
) -> OpendalEntryListResult {
    match operator_list_with_options_inner(op_handle, path, options) {
        Ok(value) => OpendalEntryListResult::ok(value),
        Err(error) => OpendalEntryListResult::from_error(error),
    }
}

fn operator_list_with_options_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::ListOptions,
) -> Result<*mut c_void, OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?;
    let options = if options.is_null() {
        opendal::options::ListOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    let entries = executor
        .block_on(handle.list_options(path, options))
        .map_err(OpenDALError::from_opendal_error)?;

    Ok(into_entry_list_ptr(entries))
}

/// List entries from `path` asynchronously with options.
///
/// The callback is invoked exactly once. On success, callback result must be
/// released with `opendal_entry_list_result_release`.
/// # Safety
///
/// - `op_handle` must be a valid operator pointer from `operator_construct`.
/// - `path` must be a valid null-terminated UTF-8 string.
/// - When `option_len > 0`, `option_keys` and `option_values` must be valid arrays.
/// - `callback` must be a valid function pointer and remain callable until invoked.
#[unsafe(no_mangle)]
pub extern "C" fn operator_list_with_options_async(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::ListOptions,
    callback: Option<ListCallback>,
    context: i64,
) -> OpendalResult {
    match operator_list_with_options_async_inner(op_handle, path, options, callback, context) {
        Ok(()) => OpendalResult::ok(),
        Err(error) => OpendalResult::from_error(error),
    }
}

fn operator_list_with_options_async_inner(
    op_handle: *const OperatorHandle,
    path: *const c_char,
    options: *const opendal::options::ListOptions,
    callback: Option<ListCallback>,
    context: i64,
) -> Result<(), OpenDALError> {
    let handle = require_op_handle(op_handle)?;
    let executor = handle.executor.clone();
    let path = require_cstr(path, "path")?.to_string();
    let callback = require_callback(callback)?;
    let options = if options.is_null() {
        opendal::options::ListOptions::default()
    } else {
        unsafe { (&*options).clone() }
    };

    let op = handle.operator();
    executor.spawn(async move {
        let result = op
            .list_options(&path, options)
            .await
            .map(into_entry_list_ptr)
            .map_err(OpenDALError::from_opendal_error);

        callback(
            context,
            match result {
                Ok(value) => OpendalEntryListResult::ok(value),
                Err(error) => OpendalEntryListResult::from_error(error),
            },
        );
    });

    Ok(())
}
