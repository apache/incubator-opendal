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

//! This is for better naming in C header file. If we use generics for Result type,
//! it will no doubt work find. However, the generics will lead to naming like
//! "opendal_result_opendal_operator", which is unacceptable. Therefore,
//! we are defining all Result types here

use super::*;

/// \brief The result type returned by opendal_operator_new() operation.
///
/// If the init logic is successful, the `op` field will be set to a valid
/// pointer, and the `error` field will be set to null. If the init logic fails, the
/// `op` field will be set to null, and the `error` field will be set to a
/// valid pointer with error code and error message.
///
/// @see opendal_operator_new()
/// @see opendal_operator
/// @see opendal_error
#[repr(C)]
pub struct opendal_result_operator_new {
    /// The pointer for operator.
    pub op: *mut opendal_operator,
    /// The error pointer for error.
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal's read operation.
///
/// The result type of read operation in opendal C binding, it contains
/// the data that the read operation returns and an NULL error.
/// If the read operation failed, the `data` fields should be a nullptr
/// and the error is not NULL.
#[repr(C)]
pub struct opendal_result_read {
    /// The byte array with length returned by read operations
    pub data: opendal_bytes,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_operator_is_exist().
///
/// The result type for opendal_operator_is_exist(), the field `is_exist`
/// contains whether the path exists, and the field `error` contains the
/// corresponding error. If successful, the `error` field is null.
///
/// \note If the opendal_operator_is_exist() fails, the `is_exist` field
/// will be set to false.
#[repr(C)]
pub struct opendal_result_is_exist {
    /// Whether the path exists
    pub is_exist: bool,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_operator_exists().
///
/// The result type for opendal_operator_exists(), the field `exists`
/// contains whether the path exists, and the field `error` contains the
/// corresponding error. If successful, the `error` field is null.
///
/// \note If the opendal_operator_exists() fails, the `exists` field
/// will be set to false.
#[repr(C)]
pub struct opendal_result_exists {
    /// Whether the path exists
    pub exists: bool,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_operator_stat().
///
/// The result type for opendal_operator_stat(), the field `meta` contains the metadata
/// of the path, the field `error` represents whether the stat operation is successful.
/// If successful, the `error` field is null.
#[repr(C)]
pub struct opendal_result_stat {
    /// The metadata output of the stat
    pub meta: *mut opendal_metadata,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_operator_list().
///
/// The result type for opendal_operator_list(), the field `lister` contains the lister
/// of the path, which is an iterator of the objects under the path. the field `error` represents
/// whether the stat operation is successful. If successful, the `error` field is null.
#[repr(C)]
pub struct opendal_result_list {
    /// The lister, used for further listing operations
    pub lister: *mut opendal_lister,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_lister_next().
/// The list entry is the list result of the list operation, the error field is the error code and error message.
/// If the operation succeeds, the error should be NULL.
///
/// \note Please notice if the lister reaches the end, both the list_entry and error will be NULL.
#[repr(C)]
pub struct opendal_result_lister_next {
    /// The next object name
    pub entry: *mut opendal_entry,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_operator_reader().
/// The result type for opendal_operator_reader(), the field `reader` contains the reader
/// of the path, which is an iterator of the objects under the path. the field `code` represents
/// whether the stat operation is successful.
#[repr(C)]
pub struct opendal_result_operator_reader {
    /// The pointer for opendal_reader
    pub reader: *mut opendal_reader,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The is the result type returned by opendal_reader_read().
/// The result type contains a size field, which is the size of the data read,
/// which is zero on error. The error field is the error code and error message.
#[repr(C)]
pub struct opendal_result_reader_read {
    /// The read size if succeed.
    pub size: usize,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_reader_seek().
/// The result type contains a pos field, which is the new position after seek,
/// which is zero on error. The error field is the error code and error message.
#[repr(C)]
pub struct opendal_result_reader_seek {
    /// New position after seek
    pub pos: u64,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_operator_writer().
/// The result type for opendal_operator_writer(), the field `writer` contains the writer
/// of the path, which is an iterator of the objects under the path. the field `code` represents
#[repr(C)]
pub struct opendal_result_operator_writer {
    /// The pointer for opendal_writer
    pub writer: *mut opendal_writer,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_writer_write().
/// The result type contains a size field, which is the size of the data written,
/// which is zero on error. The error field is the error code and error message.
#[repr(C)]
pub struct opendal_result_writer_write {
    /// The write size if succeed.
    pub size: usize,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by the metadata-returning write operations.
///
/// Returned by opendal_operator_write_with_metadata() and
/// opendal_writer_close_with_metadata(). On success the `meta` field holds the
/// metadata of the just-written object (e.g. etag, version, last modified) and
/// `error` is null; on failure `meta` is null and `error` is set.
#[repr(C)]
pub struct opendal_result_write {
    /// The metadata of the written object, or null on error.
    pub meta: *mut opendal_metadata,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_operator_copier().
///
/// Fields:
/// * `copier`: the copier used to drive a long-running copy operation by calling
///   opendal_copier_next() repeatedly; only valid when `error` is null.
/// * `error`: the error of the operation; null when the operation succeeds.
#[repr(C)]
pub struct opendal_result_operator_copier {
    /// The pointer for opendal_copier
    pub copier: *mut opendal_copier,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}

/// \brief The result type returned by opendal_copier_next().
///
/// A copy is driven by calling opendal_copier_next() repeatedly: each call
/// performs one step of the copy until the copy reports completion.
///
/// Fields:
/// * `size`: the number of bytes copied in this step; zero on error or completion.
/// * `has_next`: true when the copy made progress and should be driven again by
///   another opendal_copier_next() call; when false and `error` is null, the copy
///   has completed.
/// * `error`: the error code and message; null when the step succeeds.
#[repr(C)]
pub struct opendal_result_copier_next {
    /// The number of bytes copied in this step.
    pub size: usize,
    /// Whether the copy operation made progress and should be driven again.
    pub has_next: bool,
    /// The error, if ok, it is null
    pub error: *mut opendal_error,
}
