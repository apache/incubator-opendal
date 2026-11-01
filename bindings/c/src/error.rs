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

use ::opendal as core;
use opendal::Buffer;

use crate::types::opendal_bytes;

/// \brief The error code for all opendal APIs in C binding.
/// \todo The error handling is not complete, the error with error message will be
/// added in the future.
#[repr(C)]
pub enum opendal_code {
    /// returning it back. For example, s3 returns an internal service error.
    OPENDAL_UNEXPECTED,
    /// Underlying service doesn't support this operation.
    OPENDAL_UNSUPPORTED,
    /// The config for backend is invalid.
    OPENDAL_CONFIG_INVALID,
    /// The given path is not found.
    OPENDAL_NOT_FOUND,
    /// The given path doesn't have enough permission for this operation
    OPENDAL_PERMISSION_DENIED,
    /// The given path is a directory.
    OPENDAL_IS_A_DIRECTORY,
    /// The given path is not a directory.
    OPENDAL_NOT_A_DIRECTORY,
    /// The given path already exists thus we failed to the specified operation on it.
    OPENDAL_ALREADY_EXISTS,
    /// Requests that sent to this path is over the limit, please slow down.
    OPENDAL_RATE_LIMITED,
    /// The given file paths are same.
    OPENDAL_IS_SAME_FILE,
    /// A condition supplied through the OpenDAL operation evaluated to false.
    OPENDAL_CONDITION_NOT_MATCH,
    /// The range of the content is not satisfied.
    OPENDAL_RANGE_NOT_SATISFIED,
    /// The operation conflicts with the current or transitional state of the resource.
    OPENDAL_CONFLICT,
}

impl From<core::ErrorKind> for opendal_code {
    fn from(value: core::ErrorKind) -> Self {
        match value {
            core::ErrorKind::Unexpected => opendal_code::OPENDAL_UNEXPECTED,
            core::ErrorKind::Unsupported => opendal_code::OPENDAL_UNSUPPORTED,
            core::ErrorKind::ConfigInvalid => opendal_code::OPENDAL_CONFIG_INVALID,
            core::ErrorKind::NotFound => opendal_code::OPENDAL_NOT_FOUND,
            core::ErrorKind::PermissionDenied => opendal_code::OPENDAL_PERMISSION_DENIED,
            core::ErrorKind::IsADirectory => opendal_code::OPENDAL_IS_A_DIRECTORY,
            core::ErrorKind::NotADirectory => opendal_code::OPENDAL_NOT_A_DIRECTORY,
            core::ErrorKind::AlreadyExists => opendal_code::OPENDAL_ALREADY_EXISTS,
            core::ErrorKind::RateLimited => opendal_code::OPENDAL_RATE_LIMITED,
            core::ErrorKind::IsSameFile => opendal_code::OPENDAL_IS_SAME_FILE,
            core::ErrorKind::ConditionNotMatch => opendal_code::OPENDAL_CONDITION_NOT_MATCH,
            core::ErrorKind::RangeNotSatisfied => opendal_code::OPENDAL_RANGE_NOT_SATISFIED,
            core::ErrorKind::Conflict => opendal_code::OPENDAL_CONFLICT,
            // if this is triggered, check the [`core`] crate and add a
            // new error code accordingly
            _ => unimplemented!(
                "The newly added ErrorKind in core crate is not handled in C bindings"
            ),
        }
    }
}

/// \brief The opendal error type for C binding, containing an error code and corresponding error
/// message.
///
/// The normal operations returns a pointer to the opendal_error, and the **nullptr normally
/// represents no error has taken placed**. If any error has taken place, the caller should check
/// the error code and print the error message.
///
/// The error code is represented in opendal_code, which is an enum on different type of errors.
/// The error messages is represented in opendal_bytes, which is a non-null terminated byte array.
///
/// \note 1. The error message is on heap, so the error needs to be freed by the caller, by calling
///       opendal_error_free. 2. The error message is not null terminated, so the caller should
///       never use "%s" to print the error message.
///
/// @see opendal_code
/// @see opendal_bytes
/// @see opendal_error_free
#[repr(C)]
pub struct opendal_error {
    code: opendal_code,
    message: opendal_bytes,
}

impl opendal_error {
    /// Create a new opendal error via `core::Error`.
    ///
    /// We will call `Box::leak()` to leak this error, so the caller should be responsible for
    /// free this error.
    pub fn new(err: core::Error) -> *mut opendal_error {
        let code = opendal_code::from(err.kind());
        let message = opendal_bytes::new(Buffer::from(err.to_string()));

        Box::into_raw(Box::new(opendal_error { code, message }))
    }

    /// \brief Frees the opendal_error, ok to call on NULL
    #[no_mangle]
    pub unsafe extern "C" fn opendal_error_free(ptr: *mut opendal_error) {
        unsafe {
            if !ptr.is_null() {
                let mut err = Box::from_raw(ptr);
                opendal_bytes::opendal_bytes_free(&mut err.message);
            }
        }
    }
}
