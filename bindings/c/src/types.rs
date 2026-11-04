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
use std::ffi::{c_void, CStr, CString};
use std::os::raw::c_char;

use opendal::options;
use opendal::raw::Timestamp;
use opendal::BytesRange;
use opendal::{Buffer, Error, ErrorKind};

/// \brief Frees a heap-allocated string returned by OpenDAL C APIs.
///
/// \note Only pass pointers returned from OpenDAL APIs that transfer string ownership.
#[no_mangle]
pub unsafe extern "C" fn opendal_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}

/// \brief opendal_bytes carries raw-bytes with its length
///
/// The opendal_bytes type is a C-compatible substitute for Vec type in Rust.
/// For buffers returned by OpenDAL C APIs, call opendal_bytes_free() to free
/// the heap memory and avoid memory leaks. For caller-owned input buffers
/// passed to OpenDAL C APIs, the caller keeps ownership and must not call
/// opendal_bytes_free() on them.
///
/// @see opendal_bytes_free
#[repr(C)]
pub struct opendal_bytes {
    /// Pointing to the byte array on heap
    pub data: *mut u8,
    /// The length of the byte array
    pub len: usize,
    /// The capacity of the byte array
    pub capacity: usize,
}

impl opendal_bytes {
    pub(crate) fn empty() -> Self {
        Self {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }

    /// Construct a [`opendal_bytes`] from the Rust [`Vec`] of bytes
    pub(crate) fn new(b: Buffer) -> Self {
        let mut b = std::mem::ManuallyDrop::new(b.to_vec());
        Self {
            data: b.as_mut_ptr(),
            len: b.len(),
            capacity: b.capacity(),
        }
    }

    /// \brief Frees the heap memory used by the opendal_bytes
    #[no_mangle]
    pub unsafe extern "C" fn opendal_bytes_free(ptr: *mut opendal_bytes) {
        unsafe {
            if !ptr.is_null() {
                let bs = &mut *ptr;
                if !bs.data.is_null() {
                    drop(Vec::from_raw_parts(bs.data, bs.len, bs.capacity));
                    bs.data = std::ptr::null_mut();
                    bs.len = 0;
                    bs.capacity = 0;
                }
            }
        }
    }

    pub(crate) fn to_buffer(&self) -> opendal::Result<Buffer> {
        if self.len == 0 {
            if self.data.is_null() {
                return Ok(Buffer::new());
            }

            return Err(Error::new(
                ErrorKind::Unexpected,
                "empty opendal_bytes must have null data",
            ));
        }

        if self.data.is_null() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "non-empty opendal_bytes has null data",
            ));
        }

        let slice = unsafe { std::slice::from_raw_parts(self.data, self.len) };
        Ok(Buffer::from(bytes::Bytes::copy_from_slice(slice)))
    }
}

/// \brief The options for the list operation.
///
/// This struct carries the options for the list operation, including whether to
/// list recursively, an optional result limit, and an optional start-after key.
/// Use `opendal_list_options_new()` to construct and `opendal_list_options_free()` to free.
///
/// @see opendal_operator_list_with
/// @see opendal_list_options_new
/// @see opendal_list_options_free
/// @see opendal_list_options_set_recursive
/// @see opendal_list_options_set_limit
/// @see opendal_list_options_set_start_after
/// @see opendal_list_options_set_versions
/// @see opendal_list_options_set_deleted
#[repr(C)]
pub struct opendal_list_options {
    /// Whether to list recursively under the prefix; default false.
    pub recursive: bool,
    /// Optional hint for maximum results per request; 0 means unset.
    pub limit: usize,
    /// Optional key to start listing from; NULL means unset.
    pub start_after: *mut c_char,
    /// Include object versions when supported by version-aware backends; default false.
    pub versions: bool,
    /// Include delete markers when supported by version-aware backends; default false.
    pub deleted: bool,
}

impl opendal_list_options {
    /// \brief Construct a heap-allocated opendal_list_options with default values.
    ///
    /// @return A new opendal_list_options with all options set to their defaults.
    ///
    /// @see opendal_list_options_free
    #[no_mangle]
    pub extern "C" fn opendal_list_options_new() -> *mut Self {
        Box::into_raw(Box::new(Self {
            recursive: false,
            limit: 0,
            start_after: std::ptr::null_mut(),
            versions: false,
            deleted: false,
        }))
    }

    /// \brief Set the recursive option.
    ///
    /// @param opts The opendal_list_options to modify.
    /// @param recursive Whether to list recursively.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_list_options_set_recursive(
        opts: *mut opendal_list_options,
        recursive: bool,
    ) {
        if !opts.is_null() {
            (*opts).recursive = recursive;
        }
    }

    /// \brief Set the limit option.
    ///
    /// @param opts The opendal_list_options to modify.
    /// @param limit Maximum number of results per request; 0 means unset.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_list_options_set_limit(
        opts: *mut opendal_list_options,
        limit: usize,
    ) {
        if !opts.is_null() {
            (*opts).limit = limit;
        }
    }

    /// \brief Set the start_after option.
    ///
    /// Passes the specified key to the underlying service to start listing from.
    ///
    /// @param opts The opendal_list_options to modify.
    /// @param start_after The key to start listing from; NULL to unset.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_list_options_set_start_after(
        opts: *mut opendal_list_options,
        start_after: *const c_char,
    ) {
        if opts.is_null() {
            return;
        }

        let o = &mut *opts;
        // Free any previous value.
        if !o.start_after.is_null() {
            drop(CString::from_raw(o.start_after));
            o.start_after = std::ptr::null_mut();
        }
        if !start_after.is_null() {
            let s = CStr::from_ptr(start_after)
                .to_str()
                .expect("malformed start_after")
                .to_owned();
            o.start_after = CString::new(s).unwrap().into_raw();
        }
    }

    /// \brief Set the versions option.
    ///
    /// @param opts The opendal_list_options to modify.
    /// @param versions Whether to include object versions.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_list_options_set_versions(
        opts: *mut opendal_list_options,
        versions: bool,
    ) {
        if !opts.is_null() {
            (*opts).versions = versions;
        }
    }

    /// \brief Set the deleted option.
    ///
    /// @param opts The opendal_list_options to modify.
    /// @param deleted Whether to include delete markers.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_list_options_set_deleted(
        opts: *mut opendal_list_options,
        deleted: bool,
    ) {
        if !opts.is_null() {
            (*opts).deleted = deleted;
        }
    }

    /// \brief Free the heap memory used by opendal_list_options.
    ///
    /// @param opts The opendal_list_options to free.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_list_options_free(opts: *mut opendal_list_options) {
        if !opts.is_null() {
            let o = &mut *opts;
            if !o.start_after.is_null() {
                drop(CString::from_raw(o.start_after));
                o.start_after = std::ptr::null_mut();
            }
            drop(Box::from_raw(opts));
        }
    }
}

/// \brief The options for the delete operation.
///
/// This struct carries the options for the delete operation, including an optional
/// version string and whether to delete recursively.
/// Use `opendal_delete_options_new()` to construct and `opendal_delete_options_free()` to free.
///
/// @see opendal_operator_delete_with
/// @see opendal_delete_options_new
/// @see opendal_delete_options_free
/// @see opendal_delete_options_set_version
/// @see opendal_delete_options_set_recursive
#[repr(C)]
pub struct opendal_delete_options {
    /// Optional version string to delete a specific version; NULL means unset.
    pub version: *mut c_char,
    /// Whether to delete recursively; default false.
    pub recursive: bool,
}

impl opendal_delete_options {
    /// \brief Construct a heap-allocated opendal_delete_options with default values.
    ///
    /// @return A new opendal_delete_options with all options set to their defaults.
    ///
    /// @see opendal_delete_options_free
    #[no_mangle]
    pub extern "C" fn opendal_delete_options_new() -> *mut Self {
        Box::into_raw(Box::new(Self {
            version: std::ptr::null_mut(),
            recursive: false,
        }))
    }

    /// \brief Set the version option.
    ///
    /// @param opts The opendal_delete_options to modify.
    /// @param version The version string to delete; NULL to unset.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_delete_options_set_version(
        opts: *mut opendal_delete_options,
        version: *const c_char,
    ) {
        if opts.is_null() {
            return;
        }
        let o = &mut *opts;
        if !o.version.is_null() {
            drop(CString::from_raw(o.version));
            o.version = std::ptr::null_mut();
        }
        if !version.is_null() {
            let s = CStr::from_ptr(version)
                .to_str()
                .expect("malformed version")
                .to_owned();
            o.version = CString::new(s).unwrap().into_raw();
        }
    }

    /// \brief Set the recursive option.
    ///
    /// @param opts The opendal_delete_options to modify.
    /// @param recursive Whether to delete recursively.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_delete_options_set_recursive(
        opts: *mut opendal_delete_options,
        recursive: bool,
    ) {
        if !opts.is_null() {
            (*opts).recursive = recursive;
        }
    }

    /// \brief Free the heap memory used by opendal_delete_options.
    ///
    /// @param opts The opendal_delete_options to free.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_delete_options_free(opts: *mut opendal_delete_options) {
        if !opts.is_null() {
            let o = &mut *opts;
            if !o.version.is_null() {
                drop(CString::from_raw(o.version));
                o.version = std::ptr::null_mut();
            }
            drop(Box::from_raw(opts));
        }
    }
}

impl From<&opendal_delete_options> for options::DeleteOptions {
    fn from(value: &opendal_delete_options) -> Self {
        Self {
            version: unsafe { optional_cstr(value.version) },
            recursive: value.recursive,
            ..Default::default()
        }
    }
}

/// \brief A key-value pair for write user metadata.
#[repr(C)]
pub struct opendal_write_user_metadata_pair {
    /// The metadata key.
    pub key: *const c_char,
    /// The metadata value.
    pub value: *const c_char,
}

/// \brief The options for write operations.
///
/// Use `opendal_write_options_new()` to construct and
/// `opendal_write_options_free()` to free.
#[repr(C)]
pub struct opendal_write_options {
    /// Append data to the existing file.
    pub append: bool,
    /// Cache-Control header value.
    pub cache_control: *const c_char,
    /// Content-Type header value.
    pub content_type: *const c_char,
    /// Content-Disposition header value.
    pub content_disposition: *const c_char,
    /// Content-Encoding header value.
    pub content_encoding: *const c_char,
    /// If-Match header value.
    pub if_match: *const c_char,
    /// If-None-Match header value.
    pub if_none_match: *const c_char,
    /// Only write if target does not exist.
    pub if_not_exists: bool,
    /// Concurrent write operations. `0` means sequential writes
    pub concurrent: usize,
    /// Whether `chunk` has been set.
    pub has_chunk: bool,
    /// Chunk size for buffered writes.
    pub chunk: usize,
    /// User metadata pairs.
    pub user_metadata: *const opendal_write_user_metadata_pair,
    /// User metadata pairs length.
    pub user_metadata_len: usize,
}

impl opendal_write_options {
    /// \brief Construct a heap-allocated opendal_write_options with default values.
    #[no_mangle]
    pub extern "C" fn opendal_write_options_new() -> *mut Self {
        Box::into_raw(Box::new(Self::default()))
    }

    /// \brief Free the heap memory used by opendal_write_options.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_free(opts: *mut opendal_write_options) {
        if !opts.is_null() {
            drop(Box::from_raw(opts));
        }
    }

    /// \brief Set append mode.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_append(
        opts: *mut opendal_write_options,
        append: bool,
    ) {
        if !opts.is_null() {
            (*opts).append = append;
        }
    }

    /// \brief Set Cache-Control.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_cache_control(
        opts: *mut opendal_write_options,
        cache_control: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).cache_control = cache_control;
        }
    }

    /// \brief Set Content-Type.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_content_type(
        opts: *mut opendal_write_options,
        content_type: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).content_type = content_type;
        }
    }

    /// \brief Set Content-Disposition.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_content_disposition(
        opts: *mut opendal_write_options,
        content_disposition: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).content_disposition = content_disposition;
        }
    }

    /// \brief Set Content-Encoding.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_content_encoding(
        opts: *mut opendal_write_options,
        content_encoding: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).content_encoding = content_encoding;
        }
    }

    /// \brief Set If-Match.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_if_match(
        opts: *mut opendal_write_options,
        if_match: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).if_match = if_match;
        }
    }

    /// \brief Set If-None-Match.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_if_none_match(
        opts: *mut opendal_write_options,
        if_none_match: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).if_none_match = if_none_match;
        }
    }

    /// \brief Set if_not_exists.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_if_not_exists(
        opts: *mut opendal_write_options,
        if_not_exists: bool,
    ) {
        if !opts.is_null() {
            (*opts).if_not_exists = if_not_exists;
        }
    }

    /// \brief Set concurrent.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_concurrent(
        opts: *mut opendal_write_options,
        concurrent: usize,
    ) {
        if !opts.is_null() {
            (*opts).concurrent = concurrent;
        }
    }

    /// \brief Set chunk.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_chunk(
        opts: *mut opendal_write_options,
        chunk: usize,
    ) {
        if !opts.is_null() {
            (*opts).has_chunk = true;
            (*opts).chunk = chunk;
        }
    }

    /// \brief Set user metadata.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_write_options_set_user_metadata(
        opts: *mut opendal_write_options,
        pairs: *const opendal_write_user_metadata_pair,
        len: usize,
    ) {
        if !opts.is_null() {
            (*opts).user_metadata = pairs;
            (*opts).user_metadata_len = len;
        }
    }
}

impl Default for opendal_write_options {
    fn default() -> Self {
        Self {
            append: false,
            cache_control: std::ptr::null(),
            content_type: std::ptr::null(),
            content_disposition: std::ptr::null(),
            content_encoding: std::ptr::null(),
            if_match: std::ptr::null(),
            if_none_match: std::ptr::null(),
            if_not_exists: false,
            concurrent: 0,
            has_chunk: false,
            chunk: 0,
            user_metadata: std::ptr::null(),
            user_metadata_len: 0,
        }
    }
}

unsafe fn optional_cstr(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        Some(
            CStr::from_ptr(ptr)
                .to_str()
                .expect("malformed option")
                .to_string(),
        )
    }
}

impl From<&opendal_write_options> for options::WriteOptions {
    fn from(value: &opendal_write_options) -> Self {
        let user_metadata = if value.user_metadata.is_null() || value.user_metadata_len == 0 {
            None
        } else {
            let pairs =
                unsafe { std::slice::from_raw_parts(value.user_metadata, value.user_metadata_len) };
            let mut metadata = HashMap::with_capacity(pairs.len());
            for pair in pairs {
                if pair.key.is_null() || pair.value.is_null() {
                    continue;
                }
                let key = unsafe { CStr::from_ptr(pair.key) }
                    .to_str()
                    .expect("malformed user metadata key")
                    .to_string();
                let value = unsafe { CStr::from_ptr(pair.value) }
                    .to_str()
                    .expect("malformed user metadata value")
                    .to_string();
                metadata.insert(key, value);
            }
            Some(metadata)
        };

        Self {
            append: value.append,
            cache_control: unsafe { optional_cstr(value.cache_control) },
            content_type: unsafe { optional_cstr(value.content_type) },
            content_disposition: unsafe { optional_cstr(value.content_disposition) },
            content_encoding: unsafe { optional_cstr(value.content_encoding) },
            user_metadata,
            if_match: unsafe { optional_cstr(value.if_match) },
            if_none_match: unsafe { optional_cstr(value.if_none_match) },
            if_not_exists: value.if_not_exists,
            concurrent: value.concurrent,
            chunk: value.has_chunk.then_some(value.chunk),
            ..Default::default()
        }
    }
}

/// \brief The options for stat operations.
///
/// Use `opendal_stat_options_new()` to construct and
/// `opendal_stat_options_free()` to free.
///
/// @see opendal_operator_stat_with
#[repr(C)]
pub struct opendal_stat_options {
    /// The version of the object to stat; NULL means unset.
    pub version: *const c_char,
    /// If-Match header value; NULL means unset.
    pub if_match: *const c_char,
    /// If-None-Match header value; NULL means unset.
    pub if_none_match: *const c_char,
    /// Whether `if_modified_since` has been set.
    pub has_if_modified_since: bool,
    /// If-Modified-Since timestamp in milliseconds since the Unix epoch.
    pub if_modified_since: i64,
    /// Whether `if_unmodified_since` has been set.
    pub has_if_unmodified_since: bool,
    /// If-Unmodified-Since timestamp in milliseconds since the Unix epoch.
    pub if_unmodified_since: i64,
    /// Override the response Content-Type header; NULL means unset.
    pub override_content_type: *const c_char,
    /// Override the response Cache-Control header; NULL means unset.
    pub override_cache_control: *const c_char,
    /// Override the response Content-Disposition header; NULL means unset.
    pub override_content_disposition: *const c_char,
}

impl opendal_stat_options {
    /// \brief Construct a heap-allocated opendal_stat_options with default values.
    #[no_mangle]
    pub extern "C" fn opendal_stat_options_new() -> *mut Self {
        Box::into_raw(Box::new(Self::default()))
    }

    /// \brief Free the heap memory used by opendal_stat_options.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_stat_options_free(opts: *mut opendal_stat_options) {
        if !opts.is_null() {
            drop(Box::from_raw(opts));
        }
    }

    /// \brief Set the version.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_stat_options_set_version(
        opts: *mut opendal_stat_options,
        version: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).version = version;
        }
    }

    /// \brief Set If-Match.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_stat_options_set_if_match(
        opts: *mut opendal_stat_options,
        if_match: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).if_match = if_match;
        }
    }

    /// \brief Set If-None-Match.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_stat_options_set_if_none_match(
        opts: *mut opendal_stat_options,
        if_none_match: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).if_none_match = if_none_match;
        }
    }

    /// \brief Set If-Modified-Since in milliseconds since the Unix epoch.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_stat_options_set_if_modified_since(
        opts: *mut opendal_stat_options,
        if_modified_since: i64,
    ) {
        if !opts.is_null() {
            (*opts).has_if_modified_since = true;
            (*opts).if_modified_since = if_modified_since;
        }
    }

    /// \brief Set If-Unmodified-Since in milliseconds since the Unix epoch.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_stat_options_set_if_unmodified_since(
        opts: *mut opendal_stat_options,
        if_unmodified_since: i64,
    ) {
        if !opts.is_null() {
            (*opts).has_if_unmodified_since = true;
            (*opts).if_unmodified_since = if_unmodified_since;
        }
    }

    /// \brief Set the override Content-Type.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_stat_options_set_override_content_type(
        opts: *mut opendal_stat_options,
        override_content_type: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).override_content_type = override_content_type;
        }
    }

    /// \brief Set the override Cache-Control.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_stat_options_set_override_cache_control(
        opts: *mut opendal_stat_options,
        override_cache_control: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).override_cache_control = override_cache_control;
        }
    }

    /// \brief Set the override Content-Disposition.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_stat_options_set_override_content_disposition(
        opts: *mut opendal_stat_options,
        override_content_disposition: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).override_content_disposition = override_content_disposition;
        }
    }
}

impl Default for opendal_stat_options {
    fn default() -> Self {
        Self {
            version: std::ptr::null(),
            if_match: std::ptr::null(),
            if_none_match: std::ptr::null(),
            has_if_modified_since: false,
            if_modified_since: 0,
            has_if_unmodified_since: false,
            if_unmodified_since: 0,
            override_content_type: std::ptr::null(),
            override_cache_control: std::ptr::null(),
            override_content_disposition: std::ptr::null(),
        }
    }
}

impl From<&opendal_stat_options> for options::StatOptions {
    fn from(value: &opendal_stat_options) -> Self {
        Self {
            version: unsafe { optional_cstr(value.version) },
            if_match: unsafe { optional_cstr(value.if_match) },
            if_none_match: unsafe { optional_cstr(value.if_none_match) },
            if_modified_since: value
                .has_if_modified_since
                .then(|| Timestamp::from_millisecond(value.if_modified_since).ok())
                .flatten(),
            if_unmodified_since: value
                .has_if_unmodified_since
                .then(|| Timestamp::from_millisecond(value.if_unmodified_since).ok())
                .flatten(),
            override_content_type: unsafe { optional_cstr(value.override_content_type) },
            override_cache_control: unsafe { optional_cstr(value.override_cache_control) },
            override_content_disposition: unsafe {
                optional_cstr(value.override_content_disposition)
            },
            ..Default::default()
        }
    }
}

/// \brief The options for read operations.
///
/// Use `opendal_read_options_new()` to construct and
/// `opendal_read_options_free()` to free.
#[repr(C)]
pub struct opendal_read_options {
    /// The start offset of the range to read; default 0.
    pub offset: u64,
    /// Whether `length` has been set.
    pub has_length: bool,
    /// The number of bytes to read starting from `offset`.
    pub length: u64,
    /// The version of the object to read; NULL means unset.
    pub version: *const c_char,
    /// If-Match header value; NULL means unset.
    pub if_match: *const c_char,
    /// If-None-Match header value; NULL means unset.
    pub if_none_match: *const c_char,
    /// Whether `if_modified_since` has been set.
    pub has_if_modified_since: bool,
    /// If-Modified-Since condition, in Unix milliseconds.
    pub if_modified_since: i64,
    /// Whether `if_unmodified_since` has been set.
    pub has_if_unmodified_since: bool,
    /// If-Unmodified-Since condition, in Unix milliseconds.
    pub if_unmodified_since: i64,
    /// Concurrent read operations. `0` means sequential reads.
    pub concurrent: usize,
    /// Whether `chunk` has been set.
    pub has_chunk: bool,
    /// Chunk size for each read request.
    pub chunk: usize,
    /// Whether `gap` has been set.
    pub has_gap: bool,
    /// Gap size for merging nearby range reads.
    pub gap: usize,
    /// Whether `content_length_hint` has been set.
    pub has_content_length_hint: bool,
    /// Known content length of the object, used as an execution hint to avoid
    /// extra metadata requests while planning reads.
    pub content_length_hint: u64,
    /// Override the response Content-Type header (presign only); NULL means unset.
    pub override_content_type: *const c_char,
    /// Override the response Cache-Control header (presign only); NULL means unset.
    pub override_cache_control: *const c_char,
    /// Override the response Content-Disposition header (presign only); NULL means unset.
    pub override_content_disposition: *const c_char,
}

impl Default for opendal_read_options {
    fn default() -> Self {
        Self {
            offset: 0,
            has_length: false,
            length: 0,
            version: std::ptr::null(),
            if_match: std::ptr::null(),
            if_none_match: std::ptr::null(),
            has_if_modified_since: false,
            if_modified_since: 0,
            has_if_unmodified_since: false,
            if_unmodified_since: 0,
            concurrent: 0,
            has_chunk: false,
            chunk: 0,
            has_gap: false,
            gap: 0,
            has_content_length_hint: false,
            content_length_hint: 0,
            override_content_type: std::ptr::null(),
            override_cache_control: std::ptr::null(),
            override_content_disposition: std::ptr::null(),
        }
    }
}

impl opendal_read_options {
    /// \brief Construct a heap-allocated opendal_read_options with default values.
    #[no_mangle]
    pub extern "C" fn opendal_read_options_new() -> *mut Self {
        Box::into_raw(Box::new(Self::default()))
    }

    /// \brief Free the heap memory used by opendal_read_options.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_free(opts: *mut opendal_read_options) {
        if !opts.is_null() {
            drop(Box::from_raw(opts));
        }
    }

    /// \brief Set the read range offset and length.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_range(
        opts: *mut opendal_read_options,
        offset: u64,
        length: u64,
    ) {
        if !opts.is_null() {
            (*opts).offset = offset;
            (*opts).has_length = true;
            (*opts).length = length;
        }
    }

    /// \brief Set the read range to start at `offset` and extend to the end of the file.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_range_from(
        opts: *mut opendal_read_options,
        offset: u64,
    ) {
        if !opts.is_null() {
            (*opts).offset = offset;
            (*opts).has_length = false;
            (*opts).length = 0;
        }
    }

    /// \brief Set the version of the object to read.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_version(
        opts: *mut opendal_read_options,
        version: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).version = version;
        }
    }

    /// \brief Set If-Match.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_if_match(
        opts: *mut opendal_read_options,
        if_match: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).if_match = if_match;
        }
    }

    /// \brief Set If-None-Match.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_if_none_match(
        opts: *mut opendal_read_options,
        if_none_match: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).if_none_match = if_none_match;
        }
    }

    /// \brief Set If-Modified-Since, in Unix milliseconds.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_if_modified_since(
        opts: *mut opendal_read_options,
        if_modified_since: i64,
    ) {
        if !opts.is_null() {
            (*opts).has_if_modified_since = true;
            (*opts).if_modified_since = if_modified_since;
        }
    }

    /// \brief Set If-Unmodified-Since, in Unix milliseconds.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_if_unmodified_since(
        opts: *mut opendal_read_options,
        if_unmodified_since: i64,
    ) {
        if !opts.is_null() {
            (*opts).has_if_unmodified_since = true;
            (*opts).if_unmodified_since = if_unmodified_since;
        }
    }

    /// \brief Set concurrent read operations.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_concurrent(
        opts: *mut opendal_read_options,
        concurrent: usize,
    ) {
        if !opts.is_null() {
            (*opts).concurrent = concurrent;
        }
    }

    /// \brief Set chunk size.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_chunk(
        opts: *mut opendal_read_options,
        chunk: usize,
    ) {
        if !opts.is_null() {
            (*opts).has_chunk = true;
            (*opts).chunk = chunk;
        }
    }

    /// \brief Set gap size.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_gap(
        opts: *mut opendal_read_options,
        gap: usize,
    ) {
        if !opts.is_null() {
            (*opts).has_gap = true;
            (*opts).gap = gap;
        }
    }

    /// \brief Set the known content length of the object.
    ///
    /// This is an execution hint that allows OpenDAL to avoid extra metadata
    /// requests while planning reads. It must not be used as an object identity
    /// or consistency condition.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_content_length_hint(
        opts: *mut opendal_read_options,
        content_length_hint: u64,
    ) {
        if !opts.is_null() {
            (*opts).has_content_length_hint = true;
            (*opts).content_length_hint = content_length_hint;
        }
    }

    /// \brief Set the override Content-Type (presign only).
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_override_content_type(
        opts: *mut opendal_read_options,
        override_content_type: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).override_content_type = override_content_type;
        }
    }

    /// \brief Set the override Cache-Control (presign only).
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_override_cache_control(
        opts: *mut opendal_read_options,
        override_cache_control: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).override_cache_control = override_cache_control;
        }
    }

    /// \brief Set the override Content-Disposition (presign only).
    #[no_mangle]
    pub unsafe extern "C" fn opendal_read_options_set_override_content_disposition(
        opts: *mut opendal_read_options,
        override_content_disposition: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).override_content_disposition = override_content_disposition;
        }
    }
}

impl From<&opendal_read_options> for options::ReadOptions {
    fn from(value: &opendal_read_options) -> Self {
        let range = if value.offset > 0 || value.has_length {
            let size = if value.has_length {
                Some(value.length)
            } else {
                None
            };
            BytesRange::new(value.offset, size)
        } else {
            BytesRange::default()
        };

        Self {
            range,
            version: unsafe { optional_cstr(value.version) },
            if_match: unsafe { optional_cstr(value.if_match) },
            if_none_match: unsafe { optional_cstr(value.if_none_match) },
            if_modified_since: value
                .has_if_modified_since
                .then(|| Timestamp::from_millisecond(value.if_modified_since).ok())
                .flatten(),
            if_unmodified_since: value
                .has_if_unmodified_since
                .then(|| Timestamp::from_millisecond(value.if_unmodified_since).ok())
                .flatten(),
            content_length_hint: value
                .has_content_length_hint
                .then_some(value.content_length_hint),
            concurrent: value.concurrent,
            chunk: value.has_chunk.then_some(value.chunk),
            gap: value.has_gap.then_some(value.gap),
            override_content_type: unsafe { optional_cstr(value.override_content_type) },
            override_cache_control: unsafe { optional_cstr(value.override_cache_control) },
            override_content_disposition: unsafe {
                optional_cstr(value.override_content_disposition)
            },
            ..Default::default()
        }
    }
}

/// \brief The options for creating a reader via `opendal_operator_reader_with`.
///
/// Use `opendal_reader_options_new()` to construct and
/// `opendal_reader_options_free()` to free.
#[repr(C)]
pub struct opendal_reader_options {
    /// The version of the object to read; NULL means unset.
    pub version: *const c_char,
    /// If-Match header value; NULL means unset.
    pub if_match: *const c_char,
    /// If-None-Match header value; NULL means unset.
    pub if_none_match: *const c_char,
    /// Whether `if_modified_since` has been set.
    pub has_if_modified_since: bool,
    /// If-Modified-Since condition, in Unix milliseconds.
    pub if_modified_since: i64,
    /// Whether `if_unmodified_since` has been set.
    pub has_if_unmodified_since: bool,
    /// If-Unmodified-Since condition, in Unix milliseconds.
    pub if_unmodified_since: i64,
    /// Whether `content_length_hint` has been set.
    pub has_content_length_hint: bool,
    /// Known content length of the object, used as an execution hint to avoid
    /// extra metadata requests while planning reads.
    pub content_length_hint: u64,
    /// Concurrent read operations. `0` falls back to sequential reads.
    pub concurrent: usize,
    /// Whether `chunk` has been set.
    pub has_chunk: bool,
    /// Chunk size for each read request.
    pub chunk: usize,
    /// Whether `gap` has been set.
    pub has_gap: bool,
    /// Gap size for merging nearby range reads.
    pub gap: usize,
    /// Number of prefetched byte ranges buffered during concurrent reads.
    pub prefetch: usize,
}

impl Default for opendal_reader_options {
    fn default() -> Self {
        Self {
            version: std::ptr::null(),
            if_match: std::ptr::null(),
            if_none_match: std::ptr::null(),
            has_if_modified_since: false,
            if_modified_since: 0,
            has_if_unmodified_since: false,
            if_unmodified_since: 0,
            has_content_length_hint: false,
            content_length_hint: 0,
            concurrent: 0,
            has_chunk: false,
            chunk: 0,
            has_gap: false,
            gap: 0,
            prefetch: 0,
        }
    }
}

impl opendal_reader_options {
    /// \brief Construct a heap-allocated opendal_reader_options with default values.
    #[no_mangle]
    pub extern "C" fn opendal_reader_options_new() -> *mut Self {
        Box::into_raw(Box::new(Self::default()))
    }

    /// \brief Free the heap memory used by opendal_reader_options.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_free(opts: *mut opendal_reader_options) {
        if !opts.is_null() {
            drop(Box::from_raw(opts));
        }
    }

    /// \brief Set the version of the object to read.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_set_version(
        opts: *mut opendal_reader_options,
        version: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).version = version;
        }
    }

    /// \brief Set If-Match.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_set_if_match(
        opts: *mut opendal_reader_options,
        if_match: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).if_match = if_match;
        }
    }

    /// \brief Set If-None-Match.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_set_if_none_match(
        opts: *mut opendal_reader_options,
        if_none_match: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).if_none_match = if_none_match;
        }
    }

    /// \brief Set If-Modified-Since, in Unix milliseconds.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_set_if_modified_since(
        opts: *mut opendal_reader_options,
        if_modified_since: i64,
    ) {
        if !opts.is_null() {
            (*opts).has_if_modified_since = true;
            (*opts).if_modified_since = if_modified_since;
        }
    }

    /// \brief Set If-Unmodified-Since, in Unix milliseconds.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_set_if_unmodified_since(
        opts: *mut opendal_reader_options,
        if_unmodified_since: i64,
    ) {
        if !opts.is_null() {
            (*opts).has_if_unmodified_since = true;
            (*opts).if_unmodified_since = if_unmodified_since;
        }
    }

    /// \brief Set the known content length of the object.
    ///
    /// This is an execution hint that allows OpenDAL to avoid extra metadata
    /// requests while planning reads. It must not be used as an object identity
    /// or consistency condition.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_set_content_length_hint(
        opts: *mut opendal_reader_options,
        content_length_hint: u64,
    ) {
        if !opts.is_null() {
            (*opts).has_content_length_hint = true;
            (*opts).content_length_hint = content_length_hint;
        }
    }

    /// \brief Set concurrent read operations.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_set_concurrent(
        opts: *mut opendal_reader_options,
        concurrent: usize,
    ) {
        if !opts.is_null() {
            (*opts).concurrent = concurrent;
        }
    }

    /// \brief Set chunk size.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_set_chunk(
        opts: *mut opendal_reader_options,
        chunk: usize,
    ) {
        if !opts.is_null() {
            (*opts).has_chunk = true;
            (*opts).chunk = chunk;
        }
    }

    /// \brief Set gap size.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_set_gap(
        opts: *mut opendal_reader_options,
        gap: usize,
    ) {
        if !opts.is_null() {
            (*opts).has_gap = true;
            (*opts).gap = gap;
        }
    }

    /// \brief Set the number of prefetched byte ranges buffered during concurrent reads.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_options_set_prefetch(
        opts: *mut opendal_reader_options,
        prefetch: usize,
    ) {
        if !opts.is_null() {
            (*opts).prefetch = prefetch;
        }
    }
}

impl From<&opendal_reader_options> for options::ReaderOptions {
    fn from(value: &opendal_reader_options) -> Self {
        Self {
            version: unsafe { optional_cstr(value.version) },
            if_match: unsafe { optional_cstr(value.if_match) },
            if_none_match: unsafe { optional_cstr(value.if_none_match) },
            if_modified_since: value
                .has_if_modified_since
                .then(|| Timestamp::from_millisecond(value.if_modified_since).ok())
                .flatten(),
            if_unmodified_since: value
                .has_if_unmodified_since
                .then(|| Timestamp::from_millisecond(value.if_unmodified_since).ok())
                .flatten(),
            content_length_hint: value
                .has_content_length_hint
                .then_some(value.content_length_hint),
            concurrent: value.concurrent,
            chunk: value.has_chunk.then_some(value.chunk),
            gap: value.has_gap.then_some(value.gap),
            prefetch: value.prefetch,
            ..Default::default()
        }
    }
}

/// \brief The options for copy operations.
///
/// Use `opendal_copy_options_new()` to construct and
/// `opendal_copy_options_free()` to free.
#[repr(C)]
pub struct opendal_copy_options {
    /// Only copy if target does not exist; default false.
    pub if_not_exists: bool,
    /// If-Match condition; NULL means unset.
    pub if_match: *const c_char,
    /// Source version; NULL means unset.
    pub source_version: *const c_char,
    /// Whether `source_content_length_hint` has been set.
    pub has_source_content_length_hint: bool,
    /// Known content length of the source object.
    pub source_content_length_hint: u64,
    /// Concurrent copy operations. `0` means sequential copy.
    pub concurrent: usize,
    /// Whether `chunk` has been set.
    pub has_chunk: bool,
    /// Chunk size for segmented copy operations.
    pub chunk: usize,
}

impl opendal_copy_options {
    /// \brief Construct a heap-allocated opendal_copy_options with default values.
    #[no_mangle]
    pub extern "C" fn opendal_copy_options_new() -> *mut Self {
        Box::into_raw(Box::new(Self::default()))
    }

    /// \brief Free the heap memory used by opendal_copy_options.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_copy_options_free(opts: *mut opendal_copy_options) {
        if !opts.is_null() {
            drop(Box::from_raw(opts));
        }
    }

    /// \brief Set if_not_exists.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_copy_options_set_if_not_exists(
        opts: *mut opendal_copy_options,
        if_not_exists: bool,
    ) {
        if !opts.is_null() {
            (*opts).if_not_exists = if_not_exists;
        }
    }

    /// \brief Set If-Match.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_copy_options_set_if_match(
        opts: *mut opendal_copy_options,
        if_match: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).if_match = if_match;
        }
    }

    /// \brief Set source version.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_copy_options_set_source_version(
        opts: *mut opendal_copy_options,
        source_version: *const c_char,
    ) {
        if !opts.is_null() {
            (*opts).source_version = source_version;
        }
    }

    /// \brief Set source_content_length_hint.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_copy_options_set_source_content_length_hint(
        opts: *mut opendal_copy_options,
        source_content_length_hint: u64,
    ) {
        if !opts.is_null() {
            (*opts).has_source_content_length_hint = true;
            (*opts).source_content_length_hint = source_content_length_hint;
        }
    }

    /// \brief Set concurrent.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_copy_options_set_concurrent(
        opts: *mut opendal_copy_options,
        concurrent: usize,
    ) {
        if !opts.is_null() {
            (*opts).concurrent = concurrent;
        }
    }

    /// \brief Set chunk.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_copy_options_set_chunk(
        opts: *mut opendal_copy_options,
        chunk: usize,
    ) {
        if !opts.is_null() {
            (*opts).has_chunk = true;
            (*opts).chunk = chunk;
        }
    }
}

impl Default for opendal_copy_options {
    fn default() -> Self {
        Self {
            if_not_exists: false,
            if_match: std::ptr::null(),
            source_version: std::ptr::null(),
            has_source_content_length_hint: false,
            source_content_length_hint: 0,
            concurrent: 0,
            has_chunk: false,
            chunk: 0,
        }
    }
}

impl From<&opendal_copy_options> for options::CopyOptions {
    fn from(value: &opendal_copy_options) -> Self {
        Self {
            if_not_exists: value.if_not_exists,
            if_match: unsafe { optional_cstr(value.if_match) },
            source_version: unsafe { optional_cstr(value.source_version) },
            source_content_length_hint: value
                .has_source_content_length_hint
                .then_some(value.source_content_length_hint),
            concurrent: value.concurrent,
            chunk: value.has_chunk.then_some(value.chunk),
            ..Default::default()
        }
    }
}

impl Drop for opendal_bytes {
    fn drop(&mut self) {
        unsafe {
            // Safety: the pointer is always valid
            Self::opendal_bytes_free(self);
        }
    }
}

/// \brief The configuration for the initialization of opendal_operator.
///
/// \note This is also a heap-allocated struct, please free it after you use it
///
/// @see opendal_operator_new has an example of using opendal_operator_options
/// @see opendal_operator_options_new This function construct the operator
/// @see opendal_operator_options_free This function frees the heap memory of the operator
/// @see opendal_operator_options_set This function allow you to set the options
#[repr(C)]
pub struct opendal_operator_options {
    /// The pointer to the HashMap<String, String> in the Rust code.
    /// Only touch this on judging whether it is NULL.
    inner: *mut c_void,
}

impl opendal_operator_options {
    pub(crate) fn deref(&self) -> &HashMap<String, String> {
        // Safety: the inner should never be null once constructed
        // The use-after-free is undefined behavior
        unsafe { &*(self.inner as *mut HashMap<String, String>) }
    }

    pub(crate) fn deref_mut(&mut self) -> &mut HashMap<String, String> {
        // Safety: the inner should never be null once constructed
        // The use-after-free is undefined behavior
        unsafe { &mut *(self.inner as *mut HashMap<String, String>) }
    }
}

impl opendal_operator_options {
    /// \brief Construct a heap-allocated opendal_operator_options
    ///
    /// @return An empty opendal_operator_option, which could be set by
    /// opendal_operator_option_set().
    ///
    /// @see opendal_operator_option_set
    #[no_mangle]
    pub extern "C" fn opendal_operator_options_new() -> *mut Self {
        let map: HashMap<String, String> = HashMap::default();
        let options = Self {
            inner: Box::into_raw(Box::new(map)) as _,
        };
        Box::into_raw(Box::new(options))
    }

    /// \brief Set a Key-Value pair inside opendal_operator_options
    ///
    /// # Safety
    ///
    /// This function is unsafe because it dereferences and casts the raw pointers
    /// Make sure the pointer of `key` and `value` point to a valid string.
    ///
    /// # Example
    ///
    /// ```C
    /// opendal_operator_options *options = opendal_operator_options_new();
    /// opendal_operator_options_set(options, "root", "/myroot");
    ///
    /// // .. use your opendal_operator_options
    ///
    /// opendal_operator_options_free(options);
    /// ```
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_options_set(
        &mut self,
        key: *const c_char,
        value: *const c_char,
    ) {
        let k = unsafe { std::ffi::CStr::from_ptr(key) }
            .to_str()
            .unwrap()
            .to_string();
        let v = unsafe { std::ffi::CStr::from_ptr(value) }
            .to_str()
            .unwrap()
            .to_string();
        self.deref_mut().insert(k, v);
    }

    /// \brief Free the allocated memory used by [`opendal_operator_options`]
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_options_free(ptr: *mut opendal_operator_options) {
        unsafe {
            if !ptr.is_null() {
                drop(Box::from_raw((*ptr).inner as *mut HashMap<String, String>));
                drop(Box::from_raw(ptr));
            }
        }
    }
}
