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

use std::ffi::{c_char, c_void};

use crate::error::OpenDALError;
use crate::utils::{into_string_ptr, release_c_string, release_string_pairs, string_pairs};

#[repr(C)]
pub struct OpendalPresignedRequest {
    pub method: *mut c_char,
    pub uri: *mut c_char,
    pub headers_keys: *mut *mut c_char,
    pub headers_values: *mut *mut c_char,
    pub headers_len: usize,
}

pub fn into_presigned_request_ptr(
    request: opendal::raw::PresignedRequest,
) -> Result<*mut c_void, OpenDALError> {
    // Validate every header before allocating anything, so an invalid value
    // returns an error without leaking the strings built so far.
    let headers = request
        .header()
        .iter()
        .map(|(key, value)| value.to_str().map(|value| (key.as_str(), value)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| {
            OpenDALError::from_opendal_error(opendal::Error::new(
                opendal::ErrorKind::Unexpected,
                err.to_string(),
            ))
        })?;
    let (_, headers_keys, headers_values, headers_len) = string_pairs(Some(headers));

    let request = OpendalPresignedRequest {
        method: into_string_ptr(request.method().as_str()),
        uri: into_string_ptr(request.uri().to_string()),
        headers_keys,
        headers_values,
        headers_len,
    };

    Ok(Box::into_raw(Box::new(request)) as *mut c_void)
}

/// # Safety
///
/// - `request` must be null or a pointer produced by `into_presigned_request_ptr`.
/// - This function must be called at most once per non-null pointer.
pub(crate) unsafe fn presigned_request_free(request: *mut OpendalPresignedRequest) {
    if request.is_null() {
        return;
    }

    unsafe {
        let mut request = Box::from_raw(request);
        release_c_string(&mut request.method);
        release_c_string(&mut request.uri);
        release_string_pairs(
            &mut request.headers_keys,
            &mut request.headers_values,
            &mut request.headers_len,
        );
    }
}
