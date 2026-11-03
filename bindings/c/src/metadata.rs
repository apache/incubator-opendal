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
use std::ffi::{c_char, c_void, CString};
use std::ptr;

/// \brief A user metadata key-value pair.
#[repr(C)]
pub struct opendal_metadata_user_metadata_pair {
    /// The key of the user metadata.
    pub key: *const c_char,
    /// The value of the user metadata.
    pub value: *const c_char,
}

struct opendal_metadata_user_metadata_inner {
    pairs: Vec<opendal_metadata_user_metadata_pair>,
    #[allow(dead_code)]
    keys: Vec<CString>,
    #[allow(dead_code)]
    values: Vec<CString>,
}

impl opendal_metadata_user_metadata_inner {
    fn new(user_metadata: core::UserMetadata<'_>) -> Self {
        let mut keys = Vec::with_capacity(user_metadata.len());
        let mut values = Vec::with_capacity(user_metadata.len());
        let mut pairs = Vec::with_capacity(user_metadata.len());

        for (key, value) in user_metadata {
            keys.push(CString::new(key).expect("user metadata key should not contain nul"));
            values.push(CString::new(value).expect("user metadata value should not contain nul"));

            pairs.push(opendal_metadata_user_metadata_pair {
                key: keys.last().expect("key has just been pushed").as_ptr(),
                value: values.last().expect("value has just been pushed").as_ptr(),
            });
        }

        Self {
            pairs,
            keys,
            values,
        }
    }
}

/// \brief User metadata associated with a **path**.
#[repr(C)]
pub struct opendal_metadata_user_metadata {
    /// The pointer to the user metadata in the Rust code.
    /// Only touch this on judging whether it is NULL.
    inner: *mut c_void,
}

/// \brief Carries all metadata associated with a **path**.
///
/// The metadata of the "thing" under a path. Please **only** use the opendal_metadata
/// with our provided API, e.g. opendal_metadata_content_length().
///
/// \note The metadata is also heap-allocated, please call opendal_metadata_free() on this
/// to free the heap memory.
///
/// @see opendal_metadata_free
#[repr(C)]
pub struct opendal_metadata {
    /// The pointer to the opendal::Metadata in the Rust code.
    /// Only touch this on judging whether it is NULL.
    inner: *mut c_void,
}

impl opendal_metadata {
    fn deref(&self) -> &core::Metadata {
        // Safety: the inner should never be null once constructed
        // The use-after-free is undefined behavior
        unsafe { &*(self.inner as *mut core::Metadata) }
    }
}

impl opendal_metadata {
    /// Convert a Rust core [`od::Metadata`] into a heap allocated C-compatible
    /// [`opendal_metadata`]
    pub(crate) fn new(m: core::Metadata) -> Self {
        Self {
            inner: Box::into_raw(Box::new(m)) as _,
        }
    }

    /// \brief Free the heap-allocated metadata used by opendal_metadata
    #[no_mangle]
    pub unsafe extern "C" fn opendal_metadata_free(ptr: *mut opendal_metadata) {
        unsafe {
            if !ptr.is_null() {
                drop(Box::from_raw((*ptr).inner as *mut core::Metadata));
                drop(Box::from_raw(ptr));
            }
        }
    }

    fn optional_str(v: Option<&str>) -> *mut c_char {
        match v {
            Some(v) => CString::new(v)
                .expect("metadata string should not contain nul")
                .into_raw(),
            None => ptr::null_mut(),
        }
    }

    /// \brief Return mode of the metadata: 0 for unknown, 1 for file, and 2 for dir.
    #[no_mangle]
    pub extern "C" fn opendal_metadata_mode(&self) -> u8 {
        match self.deref().mode() {
            core::EntryMode::Unknown => 0,
            core::EntryMode::FILE => 1,
            core::EntryMode::DIR => 2,
        }
    }

    /// \brief Return the content_length of the metadata
    #[no_mangle]
    pub extern "C" fn opendal_metadata_content_length(&self) -> u64 {
        self.deref().content_length()
    }

    /// \brief Return whether the path represents a file
    #[no_mangle]
    pub extern "C" fn opendal_metadata_is_file(&self) -> bool {
        self.deref().is_file()
    }

    /// \brief Return whether the path represents a directory
    #[no_mangle]
    pub extern "C" fn opendal_metadata_is_dir(&self) -> bool {
        self.deref().is_dir()
    }

    /// \brief Return whether this metadata is current.
    ///
    /// Returns 1 for current, 0 for not current, and 2 if unknown.
    #[no_mangle]
    pub extern "C" fn opendal_metadata_is_current(&self) -> u8 {
        match self.deref().is_current() {
            Some(true) => 1,
            Some(false) => 0,
            None => 2,
        }
    }

    /// \brief Return whether this metadata is deleted.
    #[no_mangle]
    pub extern "C" fn opendal_metadata_is_deleted(&self) -> bool {
        self.deref().is_deleted()
    }

    /// \brief Return the cache control of the metadata.
    ///
    /// \note: The string is on heap, free it with opendal_string_free().
    #[no_mangle]
    pub extern "C" fn opendal_metadata_cache_control(&self) -> *mut c_char {
        Self::optional_str(self.deref().cache_control())
    }

    /// \brief Return the content disposition of the metadata.
    ///
    /// \note: The string is on heap, free it with opendal_string_free().
    #[no_mangle]
    pub extern "C" fn opendal_metadata_content_disposition(&self) -> *mut c_char {
        Self::optional_str(self.deref().content_disposition())
    }

    /// \brief Return the content md5 of the metadata.
    ///
    /// \note: The string is on heap, free it with opendal_string_free().
    #[no_mangle]
    pub extern "C" fn opendal_metadata_content_md5(&self) -> *mut c_char {
        Self::optional_str(self.deref().content_md5())
    }

    /// \brief Return the content type of the metadata.
    ///
    /// \note: The string is on heap, free it with opendal_string_free().
    #[no_mangle]
    pub extern "C" fn opendal_metadata_content_type(&self) -> *mut c_char {
        Self::optional_str(self.deref().content_type())
    }

    /// \brief Return the content encoding of the metadata.
    ///
    /// \note: The string is on heap, free it with opendal_string_free().
    #[no_mangle]
    pub extern "C" fn opendal_metadata_content_encoding(&self) -> *mut c_char {
        Self::optional_str(self.deref().content_encoding())
    }

    /// \brief Return the etag of the metadata.
    ///
    /// \note: The string is on heap, free it with opendal_string_free().
    #[no_mangle]
    pub extern "C" fn opendal_metadata_etag(&self) -> *mut c_char {
        Self::optional_str(self.deref().etag())
    }

    /// \brief Return the version of the metadata.
    ///
    /// \note: The string is on heap, free it with opendal_string_free().
    #[no_mangle]
    pub extern "C" fn opendal_metadata_version(&self) -> *mut c_char {
        Self::optional_str(self.deref().version())
    }

    /// \brief Return the user metadata of the metadata.
    ///
    /// \note: The returned user metadata is on heap, free it with opendal_metadata_user_metadata_free().
    #[no_mangle]
    pub extern "C" fn opendal_metadata_get_user_metadata(
        &self,
    ) -> *mut opendal_metadata_user_metadata {
        match self.deref().user_metadata() {
            Some(user_metadata) => Box::into_raw(Box::new(opendal_metadata_user_metadata {
                inner: Box::into_raw(Box::new(opendal_metadata_user_metadata_inner::new(
                    user_metadata,
                ))) as _,
            })),
            None => ptr::null_mut(),
        }
    }

    /// \brief Return the last_modified of the metadata, in milliseconds
    ///
    /// # Example
    /// ```C
    /// // ... previously you wrote "Hello, World!" to path "/testpath"
    /// opendal_result_stat s = opendal_operator_stat(op, "/testpath");
    /// assert(s.error == NULL);
    ///
    /// opendal_metadata *meta = s.meta;
    /// assert(opendal_metadata_last_modified_ms(meta) != -1);
    /// ```
    #[no_mangle]
    pub extern "C" fn opendal_metadata_last_modified_ms(&self) -> i64 {
        let mtime = self.deref().last_modified();
        match mtime {
            None => -1,
            Some(time) => time.into_inner().as_millisecond(),
        }
    }
}

impl opendal_metadata_user_metadata {
    /// \brief Return the key-value pairs of the user metadata.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_metadata_user_metadata_pairs(
        metadata: *const Self,
    ) -> *const opendal_metadata_user_metadata_pair {
        if metadata.is_null() {
            return ptr::null();
        }

        let inner = unsafe { &*((*metadata).inner as *mut opendal_metadata_user_metadata_inner) };
        inner.pairs.as_ptr()
    }

    /// \brief Return the number of key-value pairs in the user metadata.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_metadata_user_metadata_len(metadata: *const Self) -> usize {
        if metadata.is_null() {
            return 0;
        }

        let inner = unsafe { &*((*metadata).inner as *mut opendal_metadata_user_metadata_inner) };
        inner.pairs.len()
    }

    /// \brief Free the user metadata returned by opendal_metadata_user_metadata.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_metadata_user_metadata_free(metadata: *mut Self) {
        if !metadata.is_null() {
            drop(unsafe {
                Box::from_raw((*metadata).inner as *mut opendal_metadata_user_metadata_inner)
            });
            drop(unsafe { Box::from_raw(metadata) });
        }
    }
}
