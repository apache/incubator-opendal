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

use ::opendal as od;

#[repr(C)]
#[derive(Debug)]
pub struct ByteSlice {
    data: *mut c_char,
    len: usize,
}

impl ByteSlice {
    pub fn from_vec(vec: Vec<u8>) -> Self {
        let data = vec.as_ptr() as *mut c_char;
        let len = vec.len();

        // Leak the memory to pass the ownership to Haskell
        std::mem::forget(vec);

        ByteSlice { data, len }
    }

    /// # Safety
    ///
    /// * `ptr` is a valid pointer to a `ByteSlice`.
    ///
    /// # Panics
    ///
    /// * If `ptr` is not a valid pointer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn free_byteslice(ptr: *mut c_char, len: usize) {
        unsafe {
            if !ptr.is_null() {
                drop(Vec::from_raw_parts(ptr, len, len));
            }
        }
    }
}

impl From<&mut ByteSlice> for Vec<u8> {
    fn from(val: &mut ByteSlice) -> Self {
        unsafe { Vec::from_raw_parts(val.data as *mut u8, val.len, val.len) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub enum EntryMode {
    File,
    Dir,
    Unknown,
}

#[repr(C)]
#[derive(Debug)]
pub struct Metadata {
    mode: EntryMode,
    cache_control: *const c_char,
    content_disposition: *const c_char,
    content_length: u64,
    content_md5: *const c_char,
    content_type: *const c_char,
    etag: *const c_char,
    last_modified: *const c_char,
}

impl From<od::Metadata> for Metadata {
    fn from(val: od::Metadata) -> Self {
        let mode = match val.mode() {
            od::EntryMode::FILE => EntryMode::File,
            od::EntryMode::DIR => EntryMode::Dir,
            od::EntryMode::Unknown => EntryMode::Unknown,
        };

        let cache_control = match val.cache_control() {
            Some(s) => unsafe { leak_str(s) },
            None => std::ptr::null(),
        };

        let content_disposition = match val.content_disposition() {
            Some(s) => unsafe { leak_str(s) },
            None => std::ptr::null(),
        };

        let content_length = val.content_length();

        let content_md5 = match val.content_md5() {
            Some(s) => unsafe { leak_str(s) },
            None => std::ptr::null(),
        };

        let content_type = match val.content_type() {
            Some(s) => unsafe { leak_str(s) },
            None => std::ptr::null(),
        };

        let etag = match val.etag() {
            Some(s) => unsafe { leak_str(s) },
            None => std::ptr::null(),
        };

        let last_modified = match val.last_modified() {
            Some(s) => unsafe { leak_str(s.to_string().as_str()) },
            None => std::ptr::null(),
        };

        Metadata {
            mode,
            cache_control,
            content_disposition,
            content_length,
            content_md5,
            content_type,
            etag,
            last_modified,
        }
    }
}

// Leak the memory to pass the ownership to Haskell
// Please note that haskell should free the memory after using it
pub unsafe fn leak_str(s: &str) -> *const c_char {
    let s = match CString::new(s) {
        Ok(s) => s,
        Err(_) => return std::ptr::null(),
    };

    let ptr = s.as_ptr();
    std::mem::forget(s);
    ptr
}
