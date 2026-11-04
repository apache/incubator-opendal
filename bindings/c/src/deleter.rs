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
use std::os::raw::c_char;

use super::*;

/// \brief opendal_deleter removes many paths from storage.
///
/// opendal_deleter queues paths and hands them to the service, using batch
/// deletion when the service supports it. `opendal_deleter_delete` only queues a
/// path: the delete may still be pending when the call returns, and it is
/// `opendal_deleter_flush` that hands the queue to the service and waits for
/// every queued path to be removed. Treat a path as deleted only after
/// `opendal_deleter_flush` returns NULL.
///
/// Flushing does not end the deleter's life: it stays usable afterwards, and a
/// failed flush keeps the paths it could not delete queued so the caller can
/// retry them.
///
/// Users construct a deleter by `opendal_operator_deleter` and release it with
/// `opendal_deleter_free`.
///
/// @see opendal_operator_deleter()
/// @see opendal_deleter_delete()
/// @see opendal_deleter_delete_many()
/// @see opendal_deleter_delete_with()
/// @see opendal_deleter_flush()
#[repr(C)]
pub struct opendal_deleter {
    /// The pointer to the opendal::blocking::Deleter in the Rust code.
    /// Only used to check whether the deleter is NULL.
    inner: *mut c_void,
}

impl opendal_deleter {
    fn deref_mut(&mut self) -> &mut core::blocking::Deleter {
        // Safety: the inner should never be null once constructed
        // The use-after-free is undefined behavior
        unsafe { &mut *(self.inner as *mut core::blocking::Deleter) }
    }
}

impl opendal_deleter {
    pub(crate) fn new(deleter: core::blocking::Deleter) -> Self {
        Self {
            inner: Box::into_raw(Box::new(deleter)) as _,
        }
    }

    /// \brief Queue `path` for deletion.
    ///
    /// The path is not necessarily removed when this function returns: call
    /// `opendal_deleter_flush` to hand the queue to the service and wait for the
    /// deletions to complete.
    ///
    /// @param path The designated path you want to delete
    /// @return NULL if the path is queued, otherwise it contains the error code and
    /// error message.
    ///
    /// # Safety
    ///
    /// * The memory pointed to by `path` must contain a valid nul terminator at the
    ///   end of the string.
    ///
    /// # Panic
    ///
    /// * If the `path` points to NULL, this function panics
    #[no_mangle]
    pub unsafe extern "C" fn opendal_deleter_delete(
        &mut self,
        path: *const c_char,
    ) -> *mut opendal_error {
        // Deleting with default options is the same operation, so keep one copy
        // of the path decoding and dispatch.
        self.opendal_deleter_delete_with(path, std::ptr::null())
    }

    /// \brief Queue multiple paths for deletion.
    ///
    /// This function queues every path in one call. Callers deleting a known set
    /// of paths should prefer it over calling `opendal_deleter_delete` in a loop:
    /// it crosses the C boundary once instead of once per path, and it queues the
    /// whole set inside a single blocking call rather than one per path. The paths
    /// are not necessarily removed when this function returns: call
    /// `opendal_deleter_flush` to hand the queue to the service and wait for the
    /// deletions to complete.
    ///
    /// Every path is queued with default options. Use `opendal_deleter_delete_with`
    /// for a path that needs its own version or recursive flag.
    ///
    /// Queueing stops at the first path the service rejects, so the paths after it
    /// are never queued.
    ///
    /// @param paths The designated paths you want to delete
    /// @param paths_len The number of paths in `paths`
    /// @return NULL if all paths are queued, otherwise it contains the error code
    /// and error message.
    ///
    /// # Safety
    ///
    /// * When `paths_len` is greater than zero, `paths` must point to an array of
    ///   `paths_len` pointers.
    /// * Every pointer in `paths` must point to a string with a valid nul
    ///   terminator.
    ///
    /// # Panic
    ///
    /// * If `paths` or any pointer in `paths` is NULL when `paths_len` is greater
    ///   than zero, this function panics.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_deleter_delete_many(
        &mut self,
        paths: *const *const c_char,
        paths_len: usize,
    ) -> *mut opendal_error {
        if paths_len == 0 {
            return std::ptr::null_mut();
        }

        assert!(!paths.is_null());
        let paths = std::slice::from_raw_parts(paths, paths_len)
            .iter()
            .map(|path| {
                assert!(!path.is_null());
                std::ffi::CStr::from_ptr(*path)
                    .to_str()
                    .expect("malformed path")
                    .to_owned()
            })
            .collect::<Vec<_>>();

        match self.deref_mut().delete_iter(paths) {
            Ok(()) => std::ptr::null_mut(),
            Err(e) => opendal_error::new(e),
        }
    }

    /// \brief Queue `path` for deletion with options.
    ///
    /// This is the same as `opendal_deleter_delete` but accepts an
    /// `opendal_delete_options` to delete a specific version or to delete
    /// recursively. Pass NULL to use defaults.
    ///
    /// @param path The designated path you want to delete
    /// @param opts The options for this path; pass NULL to use defaults
    /// @see opendal_delete_options
    /// @return NULL if the path is queued, otherwise it contains the error code and
    /// error message.
    ///
    /// # Safety
    ///
    /// * The memory pointed to by `path` must contain a valid nul terminator at the
    ///   end of the string.
    ///
    /// # Panic
    ///
    /// * If the `path` points to NULL, this function panics
    #[no_mangle]
    pub unsafe extern "C" fn opendal_deleter_delete_with(
        &mut self,
        path: *const c_char,
        opts: *const opendal_delete_options,
    ) -> *mut opendal_error {
        assert!(!path.is_null());
        let path = std::ffi::CStr::from_ptr(path)
            .to_str()
            .expect("malformed path");
        let delete_opts = if opts.is_null() {
            core::options::DeleteOptions::default()
        } else {
            core::options::DeleteOptions::from(&*opts)
        };

        match self.deref_mut().delete((path.to_string(), delete_opts)) {
            Ok(()) => std::ptr::null_mut(),
            Err(e) => opendal_error::new(e),
        }
    }

    /// \brief Hand the queued paths to the service and wait until they are deleted.
    ///
    /// Call this before freeing the deleter to make sure the queued deletions
    /// happened. This function flushes the queue; it does not end the deleter's
    /// life, and `opendal_deleter_free` still has to be called. The deleter stays
    /// usable afterwards, and a failed flush keeps the paths it could not delete
    /// queued, so the caller can retry them by calling this function again.
    ///
    /// @return NULL if all queued paths are deleted, otherwise it contains the error
    /// code and error message.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_deleter_flush(&mut self) -> *mut opendal_error {
        match self.deref_mut().close() {
            Ok(()) => std::ptr::null_mut(),
            Err(e) => opendal_error::new(e),
        }
    }

    /// \brief Free the heap memory used by the opendal_deleter.
    ///
    /// Freeing a deleter drops the queued paths that were never flushed: call
    /// `opendal_deleter_flush` first unless you mean to discard them.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_deleter_free(ptr: *mut opendal_deleter) {
        unsafe {
            if !ptr.is_null() {
                drop(Box::from_raw((*ptr).inner as *mut core::blocking::Deleter));
                drop(Box::from_raw(ptr));
            }
        }
    }
}
