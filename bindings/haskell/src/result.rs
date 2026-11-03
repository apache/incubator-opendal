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

use std::ffi::CString;
use std::ffi::c_char;
use std::ptr;

use ::opendal as od;

#[repr(C)]
#[derive(Debug)]
pub struct FFIResult<T> {
    code: FFIErrorCode,
    data_ptr: *mut T,
    error_message: *mut c_char,
}

#[repr(C)]
#[derive(Debug)]
pub enum FFIErrorCode {
    Ok = 0,
    FFIError = 1,
    Unexpected = 2,
    Unsupported = 3,
    ConfigInvalid = 4,
    NotFound = 5,
    PermissionDenied = 6,
    IsADirectory = 7,
    NotADirectory = 8,
    AlreadyExists = 9,
    RateLimited = 10,
    IsSameFile = 11,
    Conflict = 12,
    ConditionNotMatch = 13,
    RangeNotSatisfied = 14,
}

impl<T> FFIResult<T> {
    pub fn ok(data: T) -> Self {
        FFIResult {
            code: FFIErrorCode::Ok,
            data_ptr: Box::into_raw(Box::new(data)),
            error_message: ptr::null_mut(),
        }
    }

    pub fn err(error_message: &str) -> Self {
        let c_string = CString::new(error_message).unwrap();
        FFIResult {
            code: FFIErrorCode::FFIError,
            data_ptr: ptr::null_mut(),
            error_message: c_string.into_raw(),
        }
    }

    pub fn err_with_source(error_message: &str, source: od::Error) -> Self {
        let msg = format!("{error_message}, source error: {source}");
        let c_string = CString::new(msg).unwrap();
        FFIResult {
            code: source.kind().into(),
            data_ptr: ptr::null_mut(),
            error_message: c_string.into_raw(),
        }
    }
}

impl From<od::ErrorKind> for FFIErrorCode {
    fn from(kind: od::ErrorKind) -> Self {
        match kind {
            od::ErrorKind::Unexpected => FFIErrorCode::Unexpected,
            od::ErrorKind::Unsupported => FFIErrorCode::Unsupported,
            od::ErrorKind::ConfigInvalid => FFIErrorCode::ConfigInvalid,
            od::ErrorKind::NotFound => FFIErrorCode::NotFound,
            od::ErrorKind::PermissionDenied => FFIErrorCode::PermissionDenied,
            od::ErrorKind::IsADirectory => FFIErrorCode::IsADirectory,
            od::ErrorKind::NotADirectory => FFIErrorCode::NotADirectory,
            od::ErrorKind::AlreadyExists => FFIErrorCode::AlreadyExists,
            od::ErrorKind::RateLimited => FFIErrorCode::RateLimited,
            od::ErrorKind::IsSameFile => FFIErrorCode::IsSameFile,
            od::ErrorKind::Conflict => FFIErrorCode::Conflict,
            od::ErrorKind::ConditionNotMatch => FFIErrorCode::ConditionNotMatch,
            od::ErrorKind::RangeNotSatisfied => FFIErrorCode::RangeNotSatisfied,
            _ => FFIErrorCode::Unexpected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kind_conversion_preserves_public_error_kinds() {
        assert!(matches!(
            FFIErrorCode::from(od::ErrorKind::ConditionNotMatch),
            FFIErrorCode::ConditionNotMatch
        ));
        assert!(matches!(
            FFIErrorCode::from(od::ErrorKind::RangeNotSatisfied),
            FFIErrorCode::RangeNotSatisfied
        ));
        assert!(matches!(
            FFIErrorCode::from(od::ErrorKind::Conflict),
            FFIErrorCode::Conflict
        ));
    }
}
