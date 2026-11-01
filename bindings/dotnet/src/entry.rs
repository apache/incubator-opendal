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

use crate::metadata::{OpendalMetadata, metadata_release_fields};
use crate::utils::{into_string_ptr, release_c_string};

#[repr(C)]
/// FFI representation of an OpenDAL entry.
///
/// The path string and metadata string fields are heap-allocated and owned by
/// Rust until released via `entry_list_free`. Metadata is stored inline so a
/// list is one contiguous array instead of a pointer per entry.
pub struct OpendalEntry {
    pub path: *mut c_char,
    pub metadata: OpendalMetadata,
}

#[repr(C)]
/// FFI representation of a list of entries.
///
/// `entries` points at a contiguous array of `len` entries stored by value.
pub struct OpendalEntryList {
    pub entries: *mut OpendalEntry,
    pub len: usize,
}

impl OpendalEntry {
    pub fn from_entry(entry: opendal::Entry) -> Self {
        let (path, metadata) = entry.into_parts();

        Self {
            path: into_string_ptr(path),
            metadata: OpendalMetadata::from_metadata(metadata),
        }
    }
}

/// Convert OpenDAL entries into an owned FFI list pointer.
///
/// The returned pointer must be released by `entry_list_free`.
pub fn into_entry_list_ptr(entries: Vec<opendal::Entry>) -> *mut c_void {
    // A boxed slice always allocates exactly `len` elements, so
    // `entry_list_free` can rebuild it from the raw parts alone.
    let entries: Box<[OpendalEntry]> = entries.into_iter().map(OpendalEntry::from_entry).collect();

    let len = entries.len();
    let entries_ptr = Box::into_raw(entries) as *mut OpendalEntry;

    Box::into_raw(Box::new(OpendalEntryList {
        entries: entries_ptr,
        len,
    })) as *mut c_void
}

/// # Safety
///
/// - `list` must be null or a pointer returned by Rust as `OpendalEntryList`.
/// - Must be called at most once for the same pointer.
pub(crate) unsafe fn entry_list_free(list: *mut OpendalEntryList) {
    if list.is_null() {
        return;
    }

    unsafe {
        let list = Box::from_raw(list);
        if list.entries.is_null() {
            return;
        }

        let mut entries = Box::from_raw(std::ptr::slice_from_raw_parts_mut(list.entries, list.len));
        for entry in entries.iter_mut() {
            release_c_string(&mut entry.path);
            metadata_release_fields(&mut entry.metadata);
        }
    }
}
