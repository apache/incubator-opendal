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

use std::ffi::{c_void, CString};
use std::os::raw::c_char;

use ::opendal as core;

use super::opendal_metadata;

/// \brief opendal_list_entry is the entry under a path, which is listed from the opendal_lister
///
/// For examples, please see the comment section of opendal_operator_list()
/// @see opendal_operator_list()
/// @see opendal_list_entry_path()
/// @see opendal_list_entry_name()
#[repr(C)]
pub struct opendal_entry {
    /// The pointer to the opendal::Entry in the Rust code.
    /// Only touch this on judging whether it is NULL.
    inner: *mut c_void,
}

impl opendal_entry {
    fn deref(&self) -> &core::Entry {
        // Safety: the inner should never be null once constructed
        // The use-after-free is undefined behavior
        unsafe { &*(self.inner as *mut core::Entry) }
    }
}

impl opendal_entry {
    /// Used to convert the Rust type into C type
    pub(crate) fn new(entry: core::Entry) -> Self {
        Self {
            inner: Box::into_raw(Box::new(entry)) as _,
        }
    }

    /// \brief Path of entry.
    ///
    /// Path is relative to operator's root. Only valid in current operator.
    ///
    /// \note Free the returned string with opendal_string_free()
    #[no_mangle]
    pub unsafe extern "C" fn opendal_entry_path(&self) -> *mut c_char {
        let s = self.deref().path();
        let c_str = CString::new(s).unwrap();
        c_str.into_raw()
    }

    /// \brief Name of entry.
    ///
    /// Name is the last segment of path.
    /// If this entry is a dir, `Name` MUST endswith `/`
    /// Otherwise, `Name` MUST NOT endswith `/`.
    ///
    /// \note Free the returned string with opendal_string_free()
    #[no_mangle]
    pub unsafe extern "C" fn opendal_entry_name(&self) -> *mut c_char {
        let s = self.deref().name();
        let c_str = CString::new(s).unwrap();
        c_str.into_raw()
    }

    /// \brief Return the metadata associated with this entry.
    ///
    /// The returned metadata is heap-allocated and must be freed
    /// by the caller via opendal_metadata_free().
    #[no_mangle]
    pub unsafe extern "C" fn opendal_entry_metadata(&self) -> *mut opendal_metadata {
        let metadata = self.deref().metadata().clone();
        Box::into_raw(Box::new(opendal_metadata::new(metadata)))
    }

    /// \brief Frees the heap memory used by the opendal_list_entry
    #[no_mangle]
    pub unsafe extern "C" fn opendal_entry_free(ptr: *mut opendal_entry) {
        unsafe {
            if !ptr.is_null() {
                drop(Box::from_raw((*ptr).inner as *mut core::Entry));
                drop(Box::from_raw(ptr));
            }
        }
    }
}
