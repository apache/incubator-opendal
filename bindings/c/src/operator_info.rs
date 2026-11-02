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
use std::ffi::{c_char, c_void};

use ::opendal as core;

use crate::opendal_operator;

/// \brief Metadata for **operator**, users can use this metadata to get information
/// of operator.
#[repr(C)]
pub struct opendal_operator_info {
    /// The pointer to the opendal::OperatorInfo in the Rust code.
    /// Only touch this on judging whether it is NULL.
    inner: *mut c_void,
}

impl opendal_operator_info {
    fn deref(&self) -> &core::OperatorInfo {
        // Safety: the inner should never be null once constructed
        // The use-after-free is undefined behavior
        unsafe { &*(self.inner as *mut core::OperatorInfo) }
    }
}

/// \brief Capability is used to describe what operations are supported
/// by current Operator.
#[repr(C)]
pub struct opendal_capability {
    /// If operator supports stat.
    pub stat: bool,
    /// If operator supports stat with if match.
    pub stat_with_if_match: bool,
    /// If operator supports stat with if none match.
    pub stat_with_if_none_match: bool,
    /// If operator supports stat with if modified since.
    pub stat_with_if_modified_since: bool,
    /// If operator supports stat with if unmodified since.
    pub stat_with_if_unmodified_since: bool,
    /// if operator supports stat with override cache control.
    pub stat_with_override_cache_control: bool,
    /// if operator supports stat with override content disposition.
    pub stat_with_override_content_disposition: bool,
    /// if operator supports stat with override content type.
    pub stat_with_override_content_type: bool,
    /// If operator supports stat with version.
    pub stat_with_version: bool,

    /// If operator supports read.
    pub read: bool,
    /// If operator supports read with if match.
    pub read_with_if_match: bool,
    /// If operator supports read with if none match.
    pub read_with_if_none_match: bool,
    /// if operator supports read with override cache control.
    pub read_with_override_cache_control: bool,
    /// if operator supports read with override content disposition.
    pub read_with_override_content_disposition: bool,
    /// if operator supports read with override content type.
    pub read_with_override_content_type: bool,
    /// If operator supports read with if modified since.
    pub read_with_if_modified_since: bool,
    /// If operator supports read with if unmodified since.
    pub read_with_if_unmodified_since: bool,
    /// If operator supports read with version.
    pub read_with_version: bool,

    /// If operator supports write.
    pub write: bool,
    /// If operator supports write can be called in multi times.
    pub write_can_multi: bool,
    /// If operator supports write with empty content.
    pub write_can_empty: bool,
    /// If operator supports write by append.
    pub write_can_append: bool,
    /// If operator supports write with content type.
    pub write_with_content_type: bool,
    /// If operator supports write with content disposition.
    pub write_with_content_disposition: bool,
    /// If operator supports write with content encoding.
    pub write_with_content_encoding: bool,
    /// If operator supports write with cache control.
    pub write_with_cache_control: bool,
    /// If operator supports write with if match.
    pub write_with_if_match: bool,
    /// If operator supports write with if none match.
    pub write_with_if_none_match: bool,
    /// If operator supports write with if not exists.
    pub write_with_if_not_exists: bool,
    /// If operator supports write with user metadata.
    pub write_with_user_metadata: bool,
    /// write_multi_max_size is the max size that services support in write_multi.
    ///
    /// For example, AWS S3 supports 5GiB as max in write_multi.
    ///
    /// If it is not set, this will be zero
    pub write_multi_max_size: usize,
    /// write_multi_min_size is the min size that services support in write_multi.
    ///
    /// For example, AWS S3 requires at least 5MiB in write_multi expect the last one.
    ///
    /// If it is not set, this will be zero
    pub write_multi_min_size: usize,
    /// write_total_max_size is the max size that services support in write_total.
    ///
    /// For example, Cloudflare D1 supports 1MB as max in write_total.
    ///
    /// If it is not set, this will be zero
    pub write_total_max_size: usize,

    /// If operator supports create dir.
    pub create_dir: bool,

    /// If operator supports delete.
    pub delete: bool,
    /// If operator supports delete with version.
    pub delete_with_version: bool,
    /// If operator supports delete with recursive.
    pub delete_with_recursive: bool,

    /// If operator supports copy.
    pub copy: bool,
    /// If operator supports copy with if not exists.
    pub copy_with_if_not_exists: bool,
    /// If operator supports copy with if match.
    pub copy_with_if_match: bool,
    /// If operator supports copy with source version.
    pub copy_with_source_version: bool,
    /// If operator supports copy can be split into multiple server-side tasks.
    pub copy_can_multi: bool,
    /// copy_multi_max_size is the max size supported for segmented copy tasks.
    ///
    /// If it is not set, this will be zero
    pub copy_multi_max_size: usize,
    /// copy_multi_min_size is the min size required for segmented copy tasks.
    ///
    /// If it is not set, this will be zero
    pub copy_multi_min_size: usize,

    /// If operator supports rename.
    pub rename: bool,

    /// If operator supports list.
    pub list: bool,
    /// If backend supports list with limit.
    pub list_with_limit: bool,
    /// If backend supports list with start after.
    pub list_with_start_after: bool,
    /// If backend supports list without delimiter.
    pub list_with_recursive: bool,
    /// If backend supports list with versions.
    pub list_with_versions: bool,
    /// If backend supports list with deleted.
    pub list_with_deleted: bool,

    /// If operator supports presign.
    pub presign: bool,
    /// If operator supports presign read.
    pub presign_read: bool,
    /// If operator supports presign stat.
    pub presign_stat: bool,
    /// If operator supports presign write.
    pub presign_write: bool,
    /// If operator supports presign delete.
    pub presign_delete: bool,

    /// If operator supports shared.
    pub shared: bool,
}

impl opendal_operator_info {
    /// \brief Get information of underlying accessor.
    ///
    /// # Example
    ///
    /// ```C
    /// /// suppose you have a memory-backed opendal_operator* named op
    /// char *scheme;
    /// opendal_operator_info *info = opendal_operator_info_new(op);
    ///
    /// scheme = opendal_operator_info_get_scheme(info);
    /// assert(!strcmp(scheme, "memory"));
    ///
    /// /// free the heap memory
    /// opendal_string_free(scheme);
    /// opendal_operator_info_free(info);
    /// ```
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_info_new(op: &opendal_operator) -> *mut Self {
        let info = op.deref().info();
        Box::into_raw(Box::new(Self {
            inner: Box::into_raw(Box::new(info)) as _,
        }))
    }

    /// \brief Free the heap-allocated opendal_operator_info
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_info_free(ptr: *mut Self) {
        unsafe {
            if !ptr.is_null() {
                drop(Box::from_raw((*ptr).inner as *mut core::OperatorInfo));
                drop(Box::from_raw(ptr));
            }
        }
    }

    /// \brief Return the nul-terminated operator's scheme, i.e. service
    ///
    /// \note: The string is on heap, free it with opendal_string_free()
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_info_get_scheme(&self) -> *mut c_char {
        let scheme = self.deref().scheme().to_string();
        CString::new(scheme)
            .expect("CString::new failed in opendal_operator_info_get_root")
            .into_raw()
    }

    /// \brief Return the nul-terminated operator's working root path
    ///
    /// \note: The string is on heap, free it with opendal_string_free()
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_info_get_root(&self) -> *mut c_char {
        let root = self.deref().root();
        CString::new(root)
            .expect("CString::new failed in opendal_operator_info_get_root")
            .into_raw()
    }

    /// \brief Return the nul-terminated operator backend's name, could be empty if underlying backend has no
    /// namespace concept.
    ///
    /// \note: The string is on heap, free it with opendal_string_free()
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_info_get_name(&self) -> *mut c_char {
        let name = self.deref().name();
        CString::new(name)
            .expect("CString::new failed in opendal_operator_info_get_name")
            .into_raw()
    }

    /// \brief Return the operator's capability
    #[no_mangle]
    pub unsafe extern "C" fn opendal_operator_info_get_capability(&self) -> opendal_capability {
        let cap = self.deref().capability();
        cap.into()
    }
}

impl From<core::Capability> for opendal_capability {
    fn from(value: core::Capability) -> Self {
        Self {
            stat: value.stat,
            stat_with_if_match: value.stat_with_if_match,
            stat_with_if_none_match: value.stat_with_if_none_match,
            stat_with_if_modified_since: value.stat_with_if_modified_since,
            stat_with_if_unmodified_since: value.stat_with_if_unmodified_since,
            stat_with_override_content_type: value.stat_with_override_content_type,
            stat_with_override_cache_control: value.stat_with_override_cache_control,
            stat_with_override_content_disposition: value.stat_with_override_content_disposition,
            stat_with_version: value.stat_with_version,
            read: value.read,
            read_with_if_match: value.read_with_if_match,
            read_with_if_none_match: value.read_with_if_none_match,
            read_with_override_content_type: value.read_with_override_content_type,
            read_with_override_cache_control: value.read_with_override_cache_control,
            read_with_override_content_disposition: value.read_with_override_content_disposition,
            read_with_if_modified_since: value.read_with_if_modified_since,
            read_with_if_unmodified_since: value.read_with_if_unmodified_since,
            read_with_version: value.read_with_version,
            write: value.write,
            write_can_multi: value.write_can_multi,
            write_can_empty: value.write_can_empty,
            write_can_append: value.write_can_append,
            write_with_content_type: value.write_with_content_type,
            write_with_content_disposition: value.write_with_content_disposition,
            write_with_content_encoding: value.write_with_content_encoding,
            write_with_cache_control: value.write_with_cache_control,
            write_with_if_match: value.write_with_if_match,
            write_with_if_none_match: value.write_with_if_none_match,
            write_with_if_not_exists: value.write_with_if_not_exists,
            write_with_user_metadata: value.write_with_user_metadata,
            write_multi_max_size: value.write_multi_max_size.unwrap_or(0),
            write_multi_min_size: value.write_multi_min_size.unwrap_or(0),
            write_total_max_size: value.write_total_max_size.unwrap_or(0),
            create_dir: value.create_dir,
            delete: value.delete,
            delete_with_version: value.delete_with_version,
            delete_with_recursive: value.delete_with_recursive,
            copy: value.copy,
            copy_with_if_not_exists: value.copy_with_if_not_exists,
            copy_with_if_match: value.copy_with_if_match,
            copy_with_source_version: value.copy_with_source_version,
            copy_can_multi: value.copy_can_multi,
            copy_multi_max_size: value.copy_multi_max_size.unwrap_or(0),
            copy_multi_min_size: value.copy_multi_min_size.unwrap_or(0),
            rename: value.rename,
            list: value.list,
            list_with_limit: value.list_with_limit,
            list_with_start_after: value.list_with_start_after,
            list_with_recursive: value.list_with_recursive,
            list_with_versions: value.list_with_versions,
            list_with_deleted: value.list_with_deleted,
            presign: value.presign,
            presign_read: value.presign_read,
            presign_stat: value.presign_stat,
            presign_write: value.presign_write,
            presign_delete: value.presign_delete,
            shared: value.shared,
        }
    }
}
