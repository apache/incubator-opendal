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

use std::ffi::c_void;

use crate::buffer::{OpendalReadBuffer, OpendalWriteBuffer, read_buffer_free};
use crate::entry::entry_list_free;
use crate::error::OpenDALError;
use crate::metadata::{OpendalMetadata, metadata_release_fields};
use crate::operator::operator_info_free;
use crate::presign::presigned_request_free;
use crate::utils::release_c_string;

/// Free a boxed metadata payload together with its heap-allocated fields.
///
/// # Safety
///
/// - `metadata` must be null or a pointer produced by `Box::into_raw` for an
///   `OpendalMetadata` built by `from_metadata`.
/// - Must be called at most once for the same pointer.
unsafe fn metadata_free(metadata: *mut OpendalMetadata) {
    if metadata.is_null() {
        return;
    }

    unsafe {
        let mut metadata = Box::from_raw(metadata);
        metadata_release_fields(&mut metadata);
    }
}

#[repr(C)]
/// Result for operations that only report success or failure.
pub struct OpendalResult {
    pub error: OpenDALError,
}

#[repr(C)]
/// Result for operations returning a boolean value.
pub struct OpendalBoolResult {
    /// `1` for true, `0` for false.
    pub value: u8,
    pub error: OpenDALError,
}

#[repr(C)]
/// Result for operations returning an operator handle pointer.
pub struct OpendalOperatorResult {
    pub ptr: *mut c_void,
    pub error: OpenDALError,
}

#[repr(C)]
/// Result for operations returning an options handle pointer.
pub struct OpendalOptionsResult {
    pub ptr: *mut c_void,
    pub error: OpenDALError,
}

#[repr(C)]
/// Result for operations returning an executor handle pointer.
pub struct OpendalExecutorResult {
    pub ptr: *mut c_void,
    pub error: OpenDALError,
}

#[repr(C)]
/// Result for operations returning an operator info payload pointer.
pub struct OpendalOperatorInfoResult {
    pub ptr: *mut c_void,
    pub error: OpenDALError,
}

#[repr(C)]
/// Result for operations returning a metadata payload pointer.
pub struct OpendalMetadataResult {
    pub ptr: *mut c_void,
    pub error: OpenDALError,
}

#[repr(C)]
/// Result for operations returning an entry list payload pointer.
pub struct OpendalEntryListResult {
    pub ptr: *mut c_void,
    pub error: OpenDALError,
}

#[repr(C)]
/// Result for operations returning a byte buffer payload.
pub struct OpendalReadResult {
    pub buffer: OpendalReadBuffer,
    pub error: OpenDALError,
}

#[repr(C)]
/// Result for operations returning a presigned request payload pointer.
pub struct OpendalPresignedRequestResult {
    pub ptr: *mut c_void,
    pub error: OpenDALError,
}

#[repr(C)]
/// Result for operations returning an allocated write buffer.
///
/// On success the caller owns the buffer handle; there is no dedicated
/// release API for this result because the handle's lifecycle is managed by
/// `write_buffer_free`, and the error message by `opendal_error_release`.
pub struct OpendalWriteResult {
    pub buffer: OpendalWriteBuffer,
    pub error: OpenDALError,
}

macro_rules! define_result {
    ($result_ty:ident) => {
        impl $result_ty {
            pub fn ok() -> Self {
                Self {
                    error: OpenDALError::ok(),
                }
            }

            pub fn from_error(error: OpenDALError) -> Self {
                Self { error }
            }
        }
    };

    (
        $result_ty:ident,
        field = $field:ident : $payload_ty:ty,
        error_value = $error_value:expr
    ) => {
        impl $result_ty {
            pub fn ok($field: $payload_ty) -> Self {
                Self {
                    $field,
                    error: OpenDALError::ok(),
                }
            }

            pub fn from_error(error: OpenDALError) -> Self {
                Self {
                    $field: $error_value,
                    error,
                }
            }
        }
    };
}

define_result!(OpendalResult);

define_result!(
    OpendalBoolResult,
    field = value: u8,
    error_value = 0
);

define_result!(
    OpendalOperatorResult,
    field = ptr: *mut c_void,
    error_value = std::ptr::null_mut()
);

define_result!(
    OpendalOptionsResult,
    field = ptr: *mut c_void,
    error_value = std::ptr::null_mut()
);

define_result!(
    OpendalExecutorResult,
    field = ptr: *mut c_void,
    error_value = std::ptr::null_mut()
);

define_result!(
    OpendalOperatorInfoResult,
    field = ptr: *mut c_void,
    error_value = std::ptr::null_mut()
);

define_result!(
    OpendalMetadataResult,
    field = ptr: *mut c_void,
    error_value = std::ptr::null_mut()
);

define_result!(
    OpendalEntryListResult,
    field = ptr: *mut c_void,
    error_value = std::ptr::null_mut()
);

define_result!(
    OpendalReadResult,
    field = buffer: OpendalReadBuffer,
    error_value = OpendalReadBuffer::empty()
);

define_result!(
    OpendalPresignedRequestResult,
    field = ptr: *mut c_void,
    error_value = std::ptr::null_mut()
);

define_result!(
    OpendalWriteResult,
    field = buffer: OpendalWriteBuffer,
    error_value = OpendalWriteBuffer::empty()
);

fn release_error_message(error: &mut OpenDALError) {
    // SAFETY: `OpenDALError` only carries a null message or one built by
    // `into_string_ptr`.
    unsafe { release_c_string(&mut error.message) };
}

/// Release an error message allocated by this FFI layer.
///
/// This function only releases `OpenDALError.message` and does not touch any
/// payload pointer carried by result structs.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_error_release(mut error: OpenDALError) {
    release_error_message(&mut error);
}

/// Release an operator-info result payload and its error message.
///
/// This function is idempotent for null payload pointers.
/// # Safety
///
/// - `result.ptr`, when non-null, must come from `operator_info_get`.
/// - The payload pointer must not be used after this call.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_operator_info_result_release(mut result: OpendalOperatorInfoResult) {
    if !result.ptr.is_null() {
        unsafe {
            operator_info_free(result.ptr.cast());
        }
    }

    release_error_message(&mut result.error);
}

/// Release a metadata result payload and its error message.
///
/// This function is idempotent for null payload pointers.
/// # Safety
///
/// - `result.ptr`, when non-null, must come from metadata-producing APIs in
///   this crate.
/// - The payload pointer must not be used after this call.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_metadata_result_release(mut result: OpendalMetadataResult) {
    if !result.ptr.is_null() {
        unsafe {
            metadata_free(result.ptr.cast());
        }
    }

    release_error_message(&mut result.error);
}

/// Release an entry-list result payload and its error message.
///
/// This function is idempotent for null payload pointers.
/// # Safety
///
/// - `result.ptr`, when non-null, must come from list-producing APIs in this
///   crate.
/// - The payload pointer must not be used after this call.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_entry_list_result_release(mut result: OpendalEntryListResult) {
    if !result.ptr.is_null() {
        unsafe {
            entry_list_free(result.ptr.cast());
        }
    }

    release_error_message(&mut result.error);
}

/// Release a read result buffer and its error message.
///
/// This function is idempotent for empty/null buffers.
/// # Safety
///
/// - `result.buffer` must originate from `OpendalReadBuffer::from_buffer` in this crate.
/// - The buffer must not be accessed after this call.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_read_result_release(mut result: OpendalReadResult) {
    unsafe {
        read_buffer_free(result.buffer.handle);
    }

    release_error_message(&mut result.error);
}

/// Release a presigned-request result payload and its error message.
///
/// This function is idempotent for null payload pointers.
/// # Safety
///
/// - `result.ptr`, when non-null, must come from presign-producing APIs in
///   this crate.
/// - The payload pointer must not be used after this call.
#[unsafe(no_mangle)]
pub extern "C" fn opendal_presigned_request_result_release(
    mut result: OpendalPresignedRequestResult,
) {
    if !result.ptr.is_null() {
        unsafe {
            presigned_request_free(result.ptr.cast());
        }
    }

    release_error_message(&mut result.error);
}
