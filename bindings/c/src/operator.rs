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
use std::ffi::c_void;
use std::os::raw::c_char;
use std::sync::LazyLock;

use ::opendal as core;

use super::*;

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

/// \brief Used to access almost all OpenDAL APIs. It represents an
/// operator that provides the unified interfaces provided by OpenDAL.
///
/// @see opendal_operator_new This function construct the operator
/// @see opendal_operator_free This function frees the heap memory of the operator
///
/// \note The opendal_operator actually owns a pointer to
/// an opendal::blocking::Operator, which is inside the Rust core code.
///
/// \remark You may use the field `ptr` to check whether this is a NULL
/// operator.
#[repr(C)]
pub struct opendal_operator {
    /// The pointer to the opendal::blocking::Operator in the Rust code.
    /// Only touch this on judging whether it is NULL.
    inner: *mut c_void,
}

impl opendal_operator {
    pub(crate) fn deref(&self) -> &core::blocking::Operator {
        // Safety: the inner should never be null once constructed
        // The use-after-free is undefined behavior
        unsafe { &*(self.inner as *mut core::blocking::Operator) }
    }
}

impl opendal_operator {
    /// \brief Free the heap-allocated operator pointed by opendal_operator.
    ///
    /// Please only use this for a pointer pointing at a valid opendal_operator.
    /// Calling this function on NULL does nothing, but calling this function on pointers
    /// of other type will lead to segfault.
    ///
    /// # Example
    ///
    /// ```C
    /// opendal_operator *op = opendal_operator_new("fs", NULL);
    /// // ... use this op, maybe some reads and writes
    ///
    /// // free this operator
    /// opendal_operator_free(op);
    /// ```
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn opendal_operator_free(ptr: *const opendal_operator) {
        unsafe {
            if !ptr.is_null() {
                drop(Box::from_raw((*ptr).inner as *mut core::blocking::Operator));
                drop(Box::from_raw(ptr as *mut opendal_operator));
            }
        }
    }
}

fn build_operator(
    schema: &str,
    map: HashMap<String, String>,
) -> core::Result<core::blocking::Operator> {
    core::init_default_registry();

    let op = core::Operator::via_iter(schema, map)?.layer(core::layers::RetryLayer::new());

    build_blocking_operator(op)
}

fn build_operator_with_layers(
    schema: &str,
    map: HashMap<String, String>,
    layers: *const opendal_operator_layers,
) -> core::Result<core::blocking::Operator> {
    core::init_default_registry();

    let mut op = core::Operator::via_iter(schema, map)?;
    if !layers.is_null() {
        op = unsafe { (*layers).apply(op) };
    }

    build_blocking_operator(op)
}

fn build_blocking_operator(op: core::Operator) -> core::Result<core::blocking::Operator> {
    let runtime =
        tokio::runtime::Handle::try_current().unwrap_or_else(|_| RUNTIME.handle().clone());
    let _guard = runtime.enter();
    let op = core::blocking::Operator::new(op)?;
    Ok(op)
}

fn parse_operator_options(options: *const opendal_operator_options) -> HashMap<String, String> {
    let mut map = HashMap::<String, String>::default();
    if !options.is_null() {
        unsafe {
            for (k, v) in (*options).deref() {
                map.insert(k.to_string(), v.to_string());
            }
        }
    }
    map
}

fn new_operator_result(op: core::Result<core::blocking::Operator>) -> opendal_result_operator_new {
    match op {
        Ok(op) => opendal_result_operator_new {
            op: Box::into_raw(Box::new(opendal_operator {
                inner: Box::into_raw(Box::new(op)) as _,
            })),
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_operator_new {
            op: std::ptr::null_mut(),
            error: opendal_error::new(e),
        },
    }
}

/// \brief Construct an operator based on `scheme` and `options`
///
/// Uses an array of key-value pairs to initialize the operator based on provided `scheme`
/// and `options`. For each scheme, i.e. Backend, different options could be set, you may
/// reference the [documentation](https://opendal.apache.org/docs/category/services/) for
/// each service, especially for the **Configuration Part**.
///
/// @param scheme the service scheme you want to specify, e.g. "fs", "s3"
/// @param options the pointer to the options for this operator, it could be NULL, which means no
/// option is set
/// @see opendal_operator_options
/// @return A valid opendal_result_operator_new setup with the `scheme` and `options` is the construction
/// succeeds. On success the operator field is a valid pointer to a newly allocated opendal_operator,
/// and the error field is NULL. Otherwise, the operator field is a NULL pointer and the error field.
///
/// # Example
///
/// Following is an example.
/// ```C
/// // Allocate a new options
/// opendal_operator_options *options = opendal_operator_options_new();
/// // Set the options you need
/// opendal_operator_options_set(options, "root", "/myroot");
///
/// // Construct the operator based on the options and scheme
/// opendal_result_operator_new result = opendal_operator_new("memory", options);
/// opendal_operator* op = result.op;
///
/// // you could free the options right away since the options is not used afterwards
/// opendal_operator_options_free(options);
///
/// // ... your operations
/// ```
///
/// # Safety
///
/// The only unsafe case is passing an invalid c string pointer to the `scheme` argument.
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_new(
    scheme: *const c_char,
    options: *const opendal_operator_options,
) -> opendal_result_operator_new {
    assert!(!scheme.is_null());
    let scheme = std::ffi::CStr::from_ptr(scheme)
        .to_str()
        .expect("malformed scheme");

    let map = parse_operator_options(options);

    new_operator_result(build_operator(scheme, map))
}

/// \brief Construct an operator based on scheme, options, and explicit layers.
///
/// Unlike opendal_operator_new, this function will not add any default layer.
/// Layers will be applied exactly as they were added to opendal_operator_layers.
///
/// # Safety
///
/// The only unsafe case is passing an invalid c string pointer to the scheme argument.
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_new_with_layers(
    scheme: *const c_char,
    options: *const opendal_operator_options,
    layers: *const opendal_operator_layers,
) -> opendal_result_operator_new {
    assert!(!scheme.is_null());
    let scheme = std::ffi::CStr::from_ptr(scheme)
        .to_str()
        .expect("malformed scheme");

    let map = parse_operator_options(options);

    new_operator_result(build_operator_with_layers(scheme, map, layers))
}

/// \brief Blocking write raw bytes to `path`.
///
/// Write the `bytes` into the `path` blocking by `op_ptr`.
/// Error is NULL if successful, otherwise it contains the error code and error message.
///
/// \note It is important to notice that the `bytes` that is passes in will be consumed by this
///       function. Therefore, you should not use the `bytes` after this function returns.
///
/// @param op The opendal_operator created previously
/// @param path The designated path you want to write your bytes in
/// @param bytes The opendal_byte typed bytes to be written
/// @see opendal_operator
/// @see opendal_bytes
/// @see opendal_error
/// @return NULL if succeeds, otherwise it contains the error code and error message.
///
/// # Example
///
/// Following is an example
/// ```C
/// //...prepare your opendal_operator, named op for example
///
/// // prepare your data
/// char* data = "Hello, World!";
/// opendal_bytes bytes = opendal_bytes { .data = (uint8_t*)data, .len = 13 };
///
/// // now you can write!
/// opendal_error *err = opendal_operator_write(op, "/testpath", bytes);
///
/// // Assert that this succeeds
/// assert(err == NULL);
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
/// * If `bytes.len` is greater than 0, `bytes.data` must point to at least
///   `bytes.len` valid bytes. If `bytes.len` is 0, `bytes.data` must be NULL.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_write(
    op: &opendal_operator,
    path: *const c_char,
    bytes: &opendal_bytes,
) -> *mut opendal_error {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let bytes = match bytes.to_buffer() {
        Ok(bytes) => bytes,
        Err(e) => return opendal_error::new(e),
    };
    match op.deref().write(path, bytes) {
        Ok(_) => std::ptr::null_mut(),
        Err(e) => opendal_error::new(e),
    }
}

/// \brief Blocking write raw bytes to `path` with options.
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_write_with(
    op: &opendal_operator,
    path: *const c_char,
    bytes: &opendal_bytes,
    opts: *const opendal_write_options,
) -> *mut opendal_error {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let opts = if opts.is_null() {
        core::options::WriteOptions::default()
    } else {
        (&*opts).into()
    };
    let bytes = match bytes.to_buffer() {
        Ok(bytes) => bytes,
        Err(e) => return opendal_error::new(e),
    };
    match op.deref().write_options(path, bytes, opts) {
        Ok(_) => std::ptr::null_mut(),
        Err(e) => opendal_error::new(e),
    }
}

/// \brief Blocking write raw bytes to `path`, returning the written object's metadata.
///
/// Like `opendal_operator_write_with`, but on success returns the metadata of the
/// just-written object (e.g. etag, version, last modified) instead of discarding it.
/// A NULL `opts` is treated as the default options, behaving like a plain write.
///
/// @param op The opendal_operator created previously
/// @param path The designated path where the data will be written
/// @param bytes The data to write
/// @param opts The write options, or NULL to use the defaults
/// @see opendal_operator
/// @see opendal_write_options
/// @see opendal_result_write
/// @return Returns opendal_result_write, containing the metadata and an opendal_error.
/// If the operation succeeds, the `meta` field holds the metadata and the `error` field
/// is null. Otherwise, the `meta` will be null and the `error` will be set correspondingly.
///
/// \note The returned metadata must be freed with opendal_metadata_free().
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
/// * The `bytes` provided has valid byte in the `data` field and the `len` field is set
///   correctly.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_write_with_metadata(
    op: &opendal_operator,
    path: *const c_char,
    bytes: &opendal_bytes,
    opts: *const opendal_write_options,
) -> opendal_result_write {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let opts = if opts.is_null() {
        core::options::WriteOptions::default()
    } else {
        (&*opts).into()
    };
    let bytes = match bytes.to_buffer() {
        Ok(bytes) => bytes,
        Err(e) => {
            return opendal_result_write {
                meta: std::ptr::null_mut(),
                error: opendal_error::new(e),
            }
        }
    };
    match op.deref().write_options(path, bytes, opts) {
        Ok(m) => opendal_result_write {
            meta: Box::into_raw(Box::new(opendal_metadata::new(m))),
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_write {
            meta: std::ptr::null_mut(),
            error: opendal_error::new(e),
        },
    }
}

/// \brief Blocking read the data from `path`.
///
/// Read the data out from `path` blocking by operator.
///
/// @param op The opendal_operator created previously
/// @param path The path you want to read the data out
/// @see opendal_operator
/// @see opendal_result_read
/// @see opendal_error
/// @return Returns opendal_result_read, the `data` field is a pointer to a newly allocated
/// opendal_bytes, the `error` field contains the error. If the `error` is not NULL, then
/// the operation failed and the `data` field is a nullptr.
///
/// \note If the read operation succeeds, the returned opendal_bytes is newly allocated on heap.
/// After your usage of that, please call opendal_bytes_free() to free the space.
///
/// # Example
///
/// Following is an example
/// ```C
/// // ... you have write "Hello, World!" to path "/testpath"
///
/// opendal_result_read r = opendal_operator_read(op, "testpath");
/// assert(r.error == NULL);
///
/// opendal_bytes bytes = r.data;
/// assert(bytes.len == 13);
/// opendal_bytes_free(&bytes);
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_read(
    op: &opendal_operator,
    path: *const c_char,
) -> opendal_result_read {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    match op.deref().read(path) {
        Ok(b) => opendal_result_read {
            data: opendal_bytes::new(b),
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_read {
            data: opendal_bytes::empty(),
            error: opendal_error::new(e),
        },
    }
}

/// \brief Blocking read the data from `path` with options.
///
/// Read the data out from `path` blocking by operator, using the provided
/// `opendal_read_options` to control the behavior, e.g. range, version, or
/// conditional headers.
///
/// @param op The opendal_operator created previously
/// @param path The path you want to read the data out
/// @param opts The options for the read operation; pass NULL to use defaults
/// @see opendal_operator
/// @see opendal_result_read
/// @see opendal_read_options
/// @see opendal_error
/// @return Returns opendal_result_read, the `data` field is a pointer to a newly allocated
/// opendal_bytes, the `error` field contains the error. If the `error` is not NULL, then
/// the operation failed and the `data` field is a nullptr.
///
/// \note If the read operation succeeds, the returned opendal_bytes is newly allocated on heap.
/// After your usage of that, please call opendal_bytes_free() to free the space.
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_read_with(
    op: &opendal_operator,
    path: *const c_char,
    opts: *const opendal_read_options,
) -> opendal_result_read {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let opts = if opts.is_null() {
        core::options::ReadOptions::default()
    } else {
        (&*opts).into()
    };
    match op.deref().read_options(path, opts) {
        Ok(b) => opendal_result_read {
            data: opendal_bytes::new(b),
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_read {
            data: opendal_bytes::empty(),
            error: opendal_error::new(e),
        },
    }
}

/// \brief Blocking read the data from `path`.
///
/// Read the data out from `path` blocking by operator, returns
/// an opendal_result_read with error code.
///
/// @param op The opendal_operator created previously
/// @param path The path you want to read the data out
/// @see opendal_operator
/// @see opendal_result_read
/// @see opendal_code
/// @return Returns opendal_code
///
/// \note If the read operation succeeds, the returned opendal_bytes is newly allocated on heap.
/// After your usage of that, please call opendal_bytes_free() to free the space.
///
/// # Example
///
/// Following is an example
/// ```C
/// // ... you have created an operator named op
///
/// opendal_result_operator_reader result = opendal_operator_reader(op, "/testpath");
/// assert(result.error == NULL);
/// // The reader is in result.reader
/// opendal_reader *reader = result.reader;
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_reader(
    op: &opendal_operator,
    path: *const c_char,
) -> opendal_result_operator_reader {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let reader = match op.deref().reader(path) {
        Ok(reader) => reader,
        Err(err) => {
            return opendal_result_operator_reader {
                reader: std::ptr::null_mut(),
                error: opendal_error::new(err),
            };
        }
    };

    match reader.into_std_read(..) {
        Ok(reader) => opendal_result_operator_reader {
            reader: Box::into_raw(Box::new(opendal_reader::new(reader))),
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_operator_reader {
            reader: std::ptr::null_mut(),
            error: opendal_error::new(e),
        },
    }
}

/// \brief Blocking create a reader for the specified path with options.
///
/// This function prepares a reader, applying the conditional, version and
/// concurrency options carried by `opts`. A NULL `opts` is treated as the
/// default options, behaving like `opendal_operator_reader`.
///
/// @param op The opendal_operator created previously
/// @param path The designated path where the reader will be used
/// @param opts The reader options, or NULL to use the defaults
/// @see opendal_operator
/// @see opendal_reader_options
/// @see opendal_result_operator_reader
/// @return Returns opendal_result_operator_reader, containing a reader and an opendal_error.
/// If the operation succeeds, the `reader` field holds a valid reader and the `error` field
/// is null. Otherwise, the `reader` will be null and the `error` will be set correspondingly.
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_reader_with(
    op: &opendal_operator,
    path: *const c_char,
    opts: *const opendal_reader_options,
) -> opendal_result_operator_reader {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let opts = if opts.is_null() {
        core::options::ReaderOptions::default()
    } else {
        (&*opts).into()
    };
    let reader = match op.deref().reader_options(path, opts) {
        Ok(reader) => reader,
        Err(err) => {
            return opendal_result_operator_reader {
                reader: std::ptr::null_mut(),
                error: opendal_error::new(err),
            };
        }
    };

    match reader.into_std_read(..) {
        Ok(reader) => opendal_result_operator_reader {
            reader: Box::into_raw(Box::new(opendal_reader::new(reader))),
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_operator_reader {
            reader: std::ptr::null_mut(),
            error: opendal_error::new(e),
        },
    }
}

/// \brief Blocking create a writer for the specified path.
///
/// This function prepares a writer that can be used to write data to the specified path
/// using the provided operator. If successful, it returns a valid writer; otherwise, it
/// returns an error.
///
/// @param op The opendal_operator created previously
/// @param path The designated path where the writer will be used
/// @see opendal_operator
/// @see opendal_result_operator_writer
/// @see opendal_error
/// @return Returns opendal_result_operator_writer, containing a writer and an opendal_error.
/// If the operation succeeds, the `writer` field holds a valid writer and the `error` field
/// is null. Otherwise, the `writer` will be null and the `error` will be set correspondingly.
///
/// # Example
///
/// Following is an example
/// ```C
/// //...prepare your opendal_operator, named op for example
///
/// opendal_result_operator_writer result = opendal_operator_writer(op, "/testpath");
/// assert(result.error == NULL);
/// opendal_writer *writer = result.writer;
/// // Use the writer to write data...
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_writer(
    op: &opendal_operator,
    path: *const c_char,
) -> opendal_result_operator_writer {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let writer = match op.deref().writer(path) {
        Ok(writer) => writer,
        Err(err) => {
            return opendal_result_operator_writer {
                writer: std::ptr::null_mut(),
                error: opendal_error::new(err),
            };
        }
    };

    opendal_result_operator_writer {
        writer: Box::into_raw(Box::new(opendal_writer::new(writer))),
        error: std::ptr::null_mut(),
    }
}

/// \brief Blocking create a writer for the specified path with options.
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_writer_with(
    op: &opendal_operator,
    path: *const c_char,
    opts: *const opendal_write_options,
) -> opendal_result_operator_writer {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let opts = if opts.is_null() {
        core::options::WriteOptions::default()
    } else {
        (&*opts).into()
    };
    let writer = match op.deref().writer_options(path, opts) {
        Ok(writer) => writer,
        Err(err) => {
            return opendal_result_operator_writer {
                writer: std::ptr::null_mut(),
                error: opendal_error::new(err),
            };
        }
    };

    opendal_result_operator_writer {
        writer: Box::into_raw(Box::new(opendal_writer::new(writer))),
        error: std::ptr::null_mut(),
    }
}

/// \brief Blocking delete the object in `path`.
///
/// Delete the object in `path` blocking by `op_ptr`.
/// Error is NULL if successful, otherwise it contains the error code and error message.
///
/// @param op The opendal_operator created previously
/// @param path The designated path you want to delete
/// @see opendal_operator
/// @see opendal_error
/// @return NULL if succeeds, otherwise it contains the error code and error message.
///
/// # Example
///
/// Following is an example
/// ```C
/// //...prepare your opendal_operator, named op for example
///
/// // prepare your data
/// char* data = "Hello, World!";
/// opendal_bytes bytes = opendal_bytes { .data = (uint8_t*)data, .len = 13 };
/// opendal_error *error = opendal_operator_write(op, "/testpath", bytes);
///
/// assert(error == NULL);
///
/// // now you can delete!
/// opendal_error *error = opendal_operator_delete(op, "/testpath");
///
/// // Assert that this succeeds
/// assert(error == NULL);
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_delete(
    op: &opendal_operator,
    path: *const c_char,
) -> *mut opendal_error {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    match op.deref().delete(path) {
        Ok(_) => std::ptr::null_mut(),
        Err(e) => opendal_error::new(e),
    }
}

/// \brief Blocking delete the object in `path` with options.
///
/// Delete the object in `path` blocking by `op`, using the provided `opendal_delete_options`.
/// This is similar to `opendal_operator_delete` but allows specifying a version or
/// requesting a recursive delete.
///
/// @param op The opendal_operator created previously
/// @param path The designated path you want to delete
/// @param opts The options for the delete operation; pass NULL to use defaults
/// @see opendal_delete_options
/// @return NULL if succeeds, otherwise it contains the error code and error message.
///
/// # Safety
///
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_delete_with(
    op: &opendal_operator,
    path: *const c_char,
    opts: *const opendal_delete_options,
) -> *mut opendal_error {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let delete_opts = if opts.is_null() {
        core::options::DeleteOptions::default()
    } else {
        let o = &*opts;
        let version = if o.version.is_null() {
            None
        } else {
            Some(
                std::ffi::CStr::from_ptr(o.version)
                    .to_str()
                    .expect("malformed version")
                    .to_owned(),
            )
        };
        core::options::DeleteOptions {
            version,
            recursive: o.recursive,
            if_match: None,
            ..Default::default()
        }
    };
    match op.deref().delete_options(path, delete_opts) {
        Ok(_) => std::ptr::null_mut(),
        Err(e) => opendal_error::new(e),
    }
}

/// \brief Check whether the path exists.
///
/// If the operation succeeds, no matter the path exists or not,
/// the error should be a nullptr. Otherwise, the field `is_exist`
/// is filled with false, and the error is set
///
/// @param op The opendal_operator created previously
/// @param path The path you want to check existence
/// @see opendal_operator
/// @see opendal_result_is_exist
/// @see opendal_error
/// @return Returns opendal_result_is_exist, the `is_exist` field contains whether the path exists.
/// However, it the operation fails, the `is_exist` will contain false and the error will be set.
///
/// # Example
///
/// ```C
/// // .. you previously wrote some data to path "/mytest/obj"
/// opendal_result_is_exist e = opendal_operator_is_exist(op, "/mytest/obj");
/// assert(e.error == NULL);
/// assert(e.is_exist);
///
/// // but you previously did **not** write any data to path "/yourtest/obj"
/// opendal_result_is_exist e = opendal_operator_is_exist(op, "/yourtest/obj");
/// assert(e.error == NULL);
/// assert(!e.is_exist);
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
#[cfg_attr(cbindgen, cbindgen::ignore)]
pub unsafe extern "C" fn opendal_operator_is_exist(
    op: &opendal_operator,
    path: *const c_char,
) -> opendal_result_is_exist {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    match op.deref().exists(path) {
        Ok(e) => opendal_result_is_exist {
            is_exist: e,
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_is_exist {
            is_exist: false,
            error: opendal_error::new(e),
        },
    }
}

/// \brief Check whether the path exists.
///
/// If the operation succeeds, no matter the path exists or not,
/// the error should be a nullptr. Otherwise, the field `exists`
/// is filled with false, and the error is set
///
/// @param op The opendal_operator created previously
/// @param path The path you want to check existence
/// @see opendal_operator
/// @see opendal_result_exists
/// @see opendal_error
/// @return Returns opendal_result_exists, the `exists` field contains whether the path exists.
/// However, it the operation fails, the `exists` will contain false and the error will be set.
///
/// # Example
///
/// ```C
/// // .. you previously wrote some data to path "/mytest/obj"
/// opendal_result_exists e = opendal_operator_exists(op, "/mytest/obj");
/// assert(e.error == NULL);
/// assert(e.exists);
///
/// // but you previously did **not** write any data to path "/yourtest/obj"
/// opendal_result_exists e = opendal_operator_exists(op, "/yourtest/obj");
/// assert(e.error == NULL);
/// assert(!e.exists);
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_exists(
    op: &opendal_operator,
    path: *const c_char,
) -> opendal_result_exists {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    match op.deref().exists(path) {
        Ok(e) => opendal_result_exists {
            exists: e,
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_exists {
            exists: false,
            error: opendal_error::new(e),
        },
    }
}

/// \brief Stat the path, return its metadata.
///
/// Error is NULL if successful, otherwise it contains the error code and error message.
///
/// @param op The opendal_operator created previously
/// @param path The path you want to stat
/// @see opendal_operator
/// @see opendal_result_stat
/// @see opendal_metadata
/// @return Returns opendal_result_stat, containing a metadata and an opendal_error.
/// If the operation succeeds, the `meta` field would hold a valid metadata and
/// the `error` field should hold nullptr. Otherwise, the metadata will contain a
/// NULL pointer, i.e. invalid, and the `error` will be set correspondingly.
///
/// # Example
///
/// ```C
/// // ... previously you wrote "Hello, World!" to path "/testpath"
/// opendal_result_stat s = opendal_operator_stat(op, "/testpath");
/// assert(s.error == NULL);
///
/// const opendal_metadata *meta = s.meta;
///
/// // ... you could now use your metadata, notice that please only access metadata
/// // using the APIs provided by OpenDAL
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_stat(
    op: &opendal_operator,
    path: *const c_char,
) -> opendal_result_stat {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    match op.deref().stat(path) {
        Ok(m) => opendal_result_stat {
            meta: Box::into_raw(Box::new(opendal_metadata::new(m))),
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_stat {
            meta: std::ptr::null_mut(),
            error: opendal_error::new(e),
        },
    }
}

/// \brief Blocking stat the object in `path` with options.
///
/// Stat the object in `path` with the provided `opendal_stat_options`. This is
/// similar to `opendal_operator_stat` but allows passing options such as
/// `version`, `if_match`, `if_none_match`, or response header overrides.
///
/// @param op The opendal_operator created previously
/// @param path The path you want to stat
/// @param opts The options for the stat operation; pass NULL to use defaults
/// @see opendal_operator
/// @see opendal_result_stat
/// @see opendal_stat_options
/// @return Returns opendal_result_stat, containing a metadata and an opendal_error.
///
/// # Safety
///
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_stat_with(
    op: &opendal_operator,
    path: *const c_char,
    opts: *const opendal_stat_options,
) -> opendal_result_stat {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let opts = if opts.is_null() {
        core::options::StatOptions::default()
    } else {
        (&*opts).into()
    };
    match op.deref().stat_options(path, opts) {
        Ok(m) => opendal_result_stat {
            meta: Box::into_raw(Box::new(opendal_metadata::new(m))),
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_stat {
            meta: std::ptr::null_mut(),
            error: opendal_error::new(e),
        },
    }
}

/// \brief Blocking list the objects in `path`.
///
/// List the object in `path` blocking by `op_ptr`, return a result with an
/// opendal_lister. Users should call opendal_lister_next() on the
/// lister.
///
/// @param op The opendal_operator created previously
/// @param path The designated path you want to list
/// @see opendal_lister
/// @return Returns opendal_result_list, containing a lister and an opendal_error.
/// If the operation succeeds, the `lister` field would hold a valid lister and
/// the `error` field should hold nullptr. Otherwise, the `lister`` will contain a
/// NULL pointer, i.e. invalid, and the `error` will be set correspondingly.
///
/// # Example
///
/// Following is an example
/// ```C
/// // You have written some data into some files path "root/dir1"
/// // Your opendal_operator was called op
/// opendal_result_list l = opendal_operator_list(op, "root/dir1");
/// assert(l.error == ERROR);
///
/// opendal_lister *lister = l.lister;
/// opendal_list_entry *entry;
///
/// while ((entry = opendal_lister_next(lister)) != NULL) {
///     const char* de_path = opendal_list_entry_path(entry);
///     const char* de_name = opendal_list_entry_name(entry);
///     // ...... your operations
///
///     // remember to free the entry after you are done using it
///     opendal_list_entry_free(entry);
/// }
///
/// // and remember to free the lister
/// opendal_lister_free(lister);
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_list(
    op: &opendal_operator,
    path: *const c_char,
) -> opendal_result_list {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    match op.deref().lister(path) {
        Ok(lister) => opendal_result_list {
            lister: Box::into_raw(Box::new(opendal_lister::new(lister))),
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_list {
            lister: std::ptr::null_mut(),
            error: opendal_error::new(e),
        },
    }
}

/// \brief Blocking list the objects in `path` with options.
///
/// List the objects in `path` with the provided `opendal_list_options`. This is
/// similar to `opendal_operator_list` but allows passing options such as
/// `recursive` to control the listing behavior.
///
/// @param op The opendal_operator created previously
/// @param path The designated path you want to list
/// @param opts The options for the list operation; pass NULL to use defaults
/// @see opendal_lister
/// @see opendal_list_options
/// @return Returns opendal_result_list, containing a lister and an opendal_error.
///
/// # Safety
///
/// * The memory pointed to by `path` must contain a valid null terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_list_with(
    op: &opendal_operator,
    path: *const c_char,
    opts: *const opendal_list_options,
) -> opendal_result_list {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    let list_opts = if opts.is_null() {
        core::options::ListOptions::default()
    } else {
        let o = &*opts;
        let limit = if o.limit == 0 { None } else { Some(o.limit) };
        let start_after = if o.start_after.is_null() {
            None
        } else {
            Some(
                std::ffi::CStr::from_ptr(o.start_after)
                    .to_str()
                    .expect("malformed start_after")
                    .to_owned(),
            )
        };
        core::options::ListOptions {
            recursive: o.recursive,
            limit,
            start_after,
            versions: o.versions,
            deleted: o.deleted,
        }
    };
    match op.deref().lister_options(path, list_opts) {
        Ok(lister) => opendal_result_list {
            lister: Box::into_raw(Box::new(opendal_lister::new(lister))),
            error: std::ptr::null_mut(),
        },
        Err(e) => opendal_result_list {
            lister: std::ptr::null_mut(),
            error: opendal_error::new(e),
        },
    }
}

/// \brief Blocking create the directory in `path`.
///
/// Create the directory in `path` blocking by `op_ptr`.
/// Error is NULL if successful, otherwise it contains the error code and error message.
///
/// @param op The opendal_operator created previously
/// @param path The designated directory you want to create
/// @see opendal_operator
/// @see opendal_error
/// @return NULL if succeeds, otherwise it contains the error code and error message.
///
/// # Example
///
/// Following is an example
/// ```C
/// //...prepare your opendal_operator, named op for example
///
/// // create your directory
/// opendal_error *error = opendal_operator_create_dir(op, "/testdir/");
///
/// // Assert that this succeeds
/// assert(error == NULL);
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `path` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_create_dir(
    op: &opendal_operator,
    path: *const c_char,
) -> *mut opendal_error {
    assert!(!path.is_null());
    let path = std::ffi::CStr::from_ptr(path)
        .to_str()
        .expect("malformed path");
    if let Err(err) = op.deref().create_dir(path) {
        opendal_error::new(err)
    } else {
        std::ptr::null_mut()
    }
}

/// \brief Blocking rename the object in `path`.
///
/// Rename the object in `src` to `dest` blocking by `op`.
/// Error is NULL if successful, otherwise it contains the error code and error message.
///
/// @param op The opendal_operator created previously
/// @param src The designated source path you want to rename
/// @param dest The designated destination path you want to rename
/// @see opendal_operator
/// @see opendal_error
/// @return NULL if succeeds, otherwise it contains the error code and error message.
///
/// # Example
///
/// Following is an example
/// ```C
/// //...prepare your opendal_operator, named op for example
///
/// // prepare your data
/// char* data = "Hello, World!";
/// opendal_bytes bytes = opendal_bytes { .data = (uint8_t*)data, .len = 13 };
/// opendal_error *error = opendal_operator_write(op, "/testpath", bytes);
///
/// assert(error == NULL);
///
/// // now you can rename!
/// opendal_error *error = opendal_operator_rename(op, "/testpath", "/testpath2");
///
/// // Assert that this succeeds
/// assert(error == NULL);
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `src` or `dest` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_rename(
    op: &opendal_operator,
    src: *const c_char,
    dest: *const c_char,
) -> *mut opendal_error {
    assert!(!src.is_null());
    assert!(!dest.is_null());
    let src = std::ffi::CStr::from_ptr(src)
        .to_str()
        .expect("malformed src");
    let dest = std::ffi::CStr::from_ptr(dest)
        .to_str()
        .expect("malformed dest");
    if let Err(err) = op.deref().rename(src, dest) {
        opendal_error::new(err)
    } else {
        std::ptr::null_mut()
    }
}

/// \brief Blocking copy the object in `path`.
///
/// Copy the object in `src` to `dest` blocking by `op`.
/// Error is NULL if successful, otherwise it contains the error code and error message.
///
/// @param op The opendal_operator created previously
/// @param src The designated source path you want to copy
/// @param dest The designated destination path you want to copy
/// @see opendal_operator
/// @see opendal_error
/// @return NULL if succeeds, otherwise it contains the error code and error message.
///
/// # Example
///
/// Following is an example
/// ```C
/// //...prepare your opendal_operator, named op for example
///
/// // prepare your data
/// char* data = "Hello, World!";
/// opendal_bytes bytes = opendal_bytes { .data = (uint8_t*)data, .len = 13 };
/// opendal_error *error = opendal_operator_write(op, "/testpath", bytes);
///
/// assert(error == NULL);
///
/// // now you can rename!
/// opendal_error *error = opendal_operator_copy(op, "/testpath", "/testpath2");
///
/// // Assert that this succeeds
/// assert(error == NULL);
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `path` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `src` or `dest` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_copy(
    op: &opendal_operator,
    src: *const c_char,
    dest: *const c_char,
) -> *mut opendal_error {
    assert!(!src.is_null());
    assert!(!dest.is_null());
    let src = std::ffi::CStr::from_ptr(src)
        .to_str()
        .expect("malformed src");
    let dest = std::ffi::CStr::from_ptr(dest)
        .to_str()
        .expect("malformed dest");
    if let Err(err) = op.deref().copy(src, dest) {
        opendal_error::new(err)
    } else {
        std::ptr::null_mut()
    }
}

/// \brief Blocking copy the object in `path` with options.
///
/// Copy the object from `src` to `dest` blocking by `op`, using the provided
/// `opendal_copy_options` to control the behavior, e.g. `if_not_exists` or
/// `if_match` conditions.
///
/// @param op The opendal_operator created previously
/// @param src The designated source path you want to copy
/// @param dest The designated destination path you want to copy
/// @param opts The options for the copy operation; pass NULL to use defaults
/// @see opendal_operator
/// @see opendal_copy_options
/// @see opendal_error
/// @return NULL if succeeds, otherwise it contains the error code and error message.
///
/// # Example
///
/// Following is an example
/// ```C
/// //...prepare your opendal_operator, named op for example
///
/// // prepare your data
/// char* data = "Hello, World!";
/// opendal_bytes bytes = opendal_bytes { .data = (uint8_t*)data, .len = 13 };
/// opendal_error *error = opendal_operator_write(op, "/testpath", bytes);
///
/// assert(error == NULL);
///
/// // prepare options
/// opendal_copy_options *opts = opendal_copy_options_new();
/// opendal_copy_options_set_if_not_exists(opts, true);
///
/// // now you can copy with options!
/// opendal_error *error = opendal_operator_copy_with(op, "/testpath", "/testpath2", opts);
///
/// // Assert that this succeeds
/// assert(error == NULL);
///
/// // remember to free the options
/// opendal_copy_options_free(opts);
/// ```
///
/// # Safety
///
/// It is **safe** under the cases below
/// * The memory pointed to by `src` and `dest` must contain a valid nul terminator at the end of
///   the string.
///
/// # Panic
///
/// * If the `src` or `dest` points to NULL, this function panics, i.e. exits with information
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_copy_with(
    op: &opendal_operator,
    src: *const c_char,
    dest: *const c_char,
    opts: *const opendal_copy_options,
) -> *mut opendal_error {
    assert!(!src.is_null());
    assert!(!dest.is_null());
    let src = std::ffi::CStr::from_ptr(src)
        .to_str()
        .expect("malformed src");
    let dest = std::ffi::CStr::from_ptr(dest)
        .to_str()
        .expect("malformed dest");
    let copy_opts = if opts.is_null() {
        core::options::CopyOptions::default()
    } else {
        core::options::CopyOptions::from(&*opts)
    };
    if let Err(err) = op.deref().copy_options(src, dest, copy_opts) {
        opendal_error::new(err)
    } else {
        std::ptr::null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn opendal_operator_check(op: &opendal_operator) -> *mut opendal_error {
    if let Err(err) = op.deref().check() {
        opendal_error::new(err)
    } else {
        std::ptr::null_mut()
    }
}

/// \brief Blocking create a copier to copy a file from `src` to `dest`.
///
/// The returned copier is used to complete a long-running copy operation. Call
/// `opendal_copier_next` repeatedly to make progress, and `opendal_copier_free`
/// to release it once finished.
///
/// @param op The opendal_operator created previously
/// @param src The designated source path you want to copy
/// @param dest The designated destination path you want to copy
/// @see opendal_operator
/// @see opendal_copier
/// @see opendal_result_operator_copier
/// @return opendal_result_operator_copier, containing a copier and an opendal_error.
/// If the operation succeeds, the `copier` field holds a valid copier and the `error`
/// field is null. Otherwise, the `copier` will be null and the `error` will be set
/// correspondingly.
///
/// # Safety
///
/// * The memory pointed to by `src` and `dest` must contain a valid nul terminator at the
///   end of the string.
///
/// # Panic
///
/// * If the `src` or `dest` points to NULL, this function panics
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_copier(
    op: &opendal_operator,
    src: *const c_char,
    dest: *const c_char,
) -> opendal_result_operator_copier {
    assert!(!src.is_null());
    assert!(!dest.is_null());
    let src = std::ffi::CStr::from_ptr(src)
        .to_str()
        .expect("malformed src");
    let dest = std::ffi::CStr::from_ptr(dest)
        .to_str()
        .expect("malformed dest");
    match op.deref().copier(src, dest) {
        Ok(copier) => opendal_result_operator_copier {
            copier: Box::into_raw(Box::new(opendal_copier::new(copier))),
            error: std::ptr::null_mut(),
        },
        Err(err) => opendal_result_operator_copier {
            copier: std::ptr::null_mut(),
            error: opendal_error::new(err),
        },
    }
}

/// \brief Blocking create a copier to copy a file from `src` to `dest` with options.
///
/// This is the same as `opendal_operator_copier` but accepts an `opendal_copy_options`
/// to control the behavior, e.g. `concurrent` or `chunk`. Pass NULL to use defaults.
///
/// @param op The opendal_operator created previously
/// @param src The designated source path you want to copy
/// @param dest The designated destination path you want to copy
/// @param opts The options for the copy operation; pass NULL to use defaults
/// @see opendal_operator_copier
/// @see opendal_copy_options
/// @return opendal_result_operator_copier, containing a copier and an opendal_error.
#[no_mangle]
pub unsafe extern "C" fn opendal_operator_copier_with(
    op: &opendal_operator,
    src: *const c_char,
    dest: *const c_char,
    opts: *const opendal_copy_options,
) -> opendal_result_operator_copier {
    assert!(!src.is_null());
    assert!(!dest.is_null());
    let src = std::ffi::CStr::from_ptr(src)
        .to_str()
        .expect("malformed src");
    let dest = std::ffi::CStr::from_ptr(dest)
        .to_str()
        .expect("malformed dest");
    let copy_opts = if opts.is_null() {
        core::options::CopyOptions::default()
    } else {
        core::options::CopyOptions::from(&*opts)
    };
    match op.deref().copier_options(src, dest, copy_opts) {
        Ok(copier) => opendal_result_operator_copier {
            copier: Box::into_raw(Box::new(opendal_copier::new(copier))),
            error: std::ptr::null_mut(),
        },
        Err(err) => opendal_result_operator_copier {
            copier: std::ptr::null_mut(),
            error: opendal_error::new(err),
        },
    }
}
