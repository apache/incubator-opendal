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
use std::ffi::c_void;

use super::*;

/// \brief opendal_copier completes a long-running copy operation to
/// completion in a blocking manner. opendal_copier repeatedly calls a
/// copy operation until completion.
///
/// Internally, each copy step performs a unit of work and reports progress.
/// When copy completes, `has_next` returns false.
///
/// A "step" is one backend-defined unit of work. For backends that copy in
/// multiple requests (e.g. multipart copy), one step typically copies one chunk;
/// for backends that only support single-request copy, the entire copy happens
/// in a single step. The reported byte count is best-effort: a step may report
/// 0 bytes when the backend advances its state without a reliable byte delta.
///
/// Users can construct a copier by `opendal_operator_copier` or
/// `opendal_operator_copier_with`.
///
/// @see opendal_operator_copier()
/// @see opendal_copier_next()
#[repr(C)]
pub struct opendal_copier {
    /// The pointer to the opendal::blocking::Copier in the Rust code.
    /// Only used to check whether the copier is NULL.
    inner: *mut c_void,
}

impl opendal_copier {
    fn deref_mut(&mut self) -> &mut core::blocking::Copier {
        // Safety: the inner should never be null once constructed
        // The use-after-free is undefined behavior
        unsafe { &mut *(self.inner as *mut core::blocking::Copier) }
    }
}

impl opendal_copier {
    pub(crate) fn new(copier: core::blocking::Copier) -> Self {
        Self {
            inner: Box::into_raw(Box::new(copier)) as _,
        }
    }

    /// \brief Perform one step of the copy operation.
    ///
    /// One step performs one backend-defined unit of work: typically one chunk for
    /// backends that copy in multiple requests, or the entire copy for backends that
    /// only support single-request copy.
    ///
    /// Returns the number of bytes copied in this step (best-effort; may be 0 when
    /// the backend advances without a reliable byte delta). When `has_next` is true
    /// the caller should call this function again to continue the copy. When
    /// `has_next` is false and `error` is null the copy has completed.
    ///
    /// @see opendal_operator_copier()
    #[no_mangle]
    pub unsafe extern "C" fn opendal_copier_next(&mut self) -> opendal_result_copier_next {
        match self.deref_mut().next() {
            Some(Ok(n)) => opendal_result_copier_next {
                size: n,
                has_next: true,
                error: std::ptr::null_mut(),
            },
            None => opendal_result_copier_next {
                size: 0,
                has_next: false,
                error: std::ptr::null_mut(),
            },
            Some(Err(e)) => opendal_result_copier_next {
                size: 0,
                has_next: false,
                error: opendal_error::new(e),
            },
        }
    }

    /// \brief Abort the pending copy operation.
    ///
    /// Returns NULL if the abort succeeds, otherwise it contains the error code and
    /// error message.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_copier_abort(&mut self) -> *mut opendal_error {
        if let Err(e) = self.deref_mut().abort() {
            opendal_error::new(e)
        } else {
            std::ptr::null_mut()
        }
    }

    /// \brief Free the heap memory used by the opendal_copier.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_copier_free(ptr: *mut opendal_copier) {
        unsafe {
            if !ptr.is_null() {
                drop(Box::from_raw((*ptr).inner as *mut core::blocking::Copier));
                // A use-after-free crashes on NULL instead of reading stale memory.
                (*ptr).inner = std::ptr::null_mut();
                drop(Box::from_raw(ptr));
            }
        }
    }
}
