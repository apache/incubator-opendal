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

use pyo3::prelude::*;

/// Capability defines the supported operations and their constraints for an Operator.
///
/// This structure provides a comprehensive description of an Operator's
/// capabilities, including:
///
/// - Basic operations support (read, write, delete, etc.)
/// - Advanced operation variants (conditional operations, metadata handling)
/// - Operational constraints (size limits, batch limitations)
#[pyclass(get_all, module = "opendal.capability")]
pub struct Capability {
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
    /// If operator supports stat with version.
    pub stat_with_version: bool,

    /// If the operator supports read operations.
    pub read: bool,
    /// If conditional read operations using If-Match are supported.
    pub read_with_if_match: bool,
    /// If conditional read operations using If-None-Match are supported.
    pub read_with_if_none_match: bool,
    /// If conditional read operations using If-Modified-Since are supported.
    pub read_with_if_modified_since: bool,
    /// If conditional read operations using If-Unmodified-Since are supported.
    pub read_with_if_unmodified_since: bool,
    /// If Cache-Control header override is supported during read operations.
    pub read_with_override_cache_control: bool,
    /// If Content-Disposition header can be overridden during read operations.
    pub read_with_override_content_disposition: bool,
    /// If Content-Type header override is supported during read operations.
    pub read_with_override_content_type: bool,
    /// If versions read operations are supported.
    pub read_with_version: bool,

    /// If the operator supports write operations.
    pub write: bool,
    /// If multiple write operations can be performed on the same object.
    pub write_can_multi: bool,
    /// If writing empty content is supported.
    pub write_can_empty: bool,
    /// If append operations are supported.
    pub write_can_append: bool,
    /// If Content-Type can be specified during write operations.
    pub write_with_content_type: bool,
    /// If Content-Disposition can be specified during write operations.
    pub write_with_content_disposition: bool,
    /// If Content-Encoding can be specified during write operations.
    pub write_with_content_encoding: bool,
    /// If Cache-Control can be specified during write operations.
    pub write_with_cache_control: bool,
    /// If conditional write operations using If-Match are supported.
    pub write_with_if_match: bool,
    /// If conditional write operations using If-None-Match are supported.
    pub write_with_if_none_match: bool,
    /// If write operations can be conditional on object non-existence.
    pub write_with_if_not_exists: bool,
    /// If custom user metadata can be attached during write operations.
    pub write_with_user_metadata: bool,
    /// Maximum size supported for multipart uploads.
    ///
    /// For example, AWS S3 supports up to 5GiB per part in multipart uploads.
    pub write_multi_max_size: Option<usize>,
    /// Minimum size required for multipart uploads (except for the last part).
    ///
    /// For example, AWS S3 requires at least 5MiB per part.
    pub write_multi_min_size: Option<usize>,
    /// Maximum total size supported for write operations.
    ///
    /// For example, Cloudflare D1 has a 1MB total size limit.
    pub write_total_max_size: Option<usize>,

    /// If operator supports create dir.
    pub create_dir: bool,

    /// If operator supports delete.
    pub delete: bool,
    /// If delete operations with version are supported.
    pub delete_with_version: bool,
    /// If recursive delete operations are supported.
    pub delete_with_recursive: bool,

    /// If operator supports copy.
    pub copy: bool,

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

impl Capability {
    pub fn new(capability: opendal::Capability) -> Self {
        Self {
            stat: capability.stat,
            stat_with_if_match: capability.stat_with_if_match,
            stat_with_if_none_match: capability.stat_with_if_none_match,
            stat_with_if_modified_since: capability.stat_with_if_modified_since,
            stat_with_if_unmodified_since: capability.stat_with_if_unmodified_since,
            stat_with_version: capability.stat_with_version,
            read: capability.read,
            read_with_if_match: capability.read_with_if_match,
            read_with_if_none_match: capability.read_with_if_none_match,
            read_with_override_cache_control: capability.read_with_override_cache_control,
            read_with_override_content_disposition: capability
                .read_with_override_content_disposition,
            read_with_override_content_type: capability.read_with_override_content_type,
            read_with_if_modified_since: capability.read_with_if_modified_since,
            read_with_if_unmodified_since: capability.read_with_if_unmodified_since,
            read_with_version: capability.read_with_version,
            write: capability.write,
            write_can_multi: capability.write_can_multi,
            write_can_empty: capability.write_can_empty,
            write_can_append: capability.write_can_append,
            write_with_content_type: capability.write_with_content_type,
            write_with_content_disposition: capability.write_with_content_disposition,
            write_with_cache_control: capability.write_with_cache_control,
            write_multi_max_size: capability.write_multi_max_size,
            write_multi_min_size: capability.write_multi_min_size,
            write_total_max_size: capability.write_total_max_size,
            write_with_content_encoding: capability.write_with_content_encoding,
            write_with_if_match: capability.write_with_if_match,
            write_with_if_none_match: capability.write_with_if_none_match,
            write_with_if_not_exists: capability.write_with_if_not_exists,
            write_with_user_metadata: capability.write_with_user_metadata,
            create_dir: capability.create_dir,
            delete: capability.delete,
            delete_with_version: capability.delete_with_version,
            delete_with_recursive: capability.delete_with_recursive,
            copy: capability.copy,
            rename: capability.rename,
            list: capability.list,
            list_with_limit: capability.list_with_limit,
            list_with_start_after: capability.list_with_start_after,
            list_with_recursive: capability.list_with_recursive,
            list_with_versions: capability.list_with_versions,
            list_with_deleted: capability.list_with_deleted,
            presign: capability.presign,
            presign_read: capability.presign_read,
            presign_stat: capability.presign_stat,
            presign_write: capability.presign_write,
            presign_delete: capability.presign_delete,
            shared: capability.shared,
        }
    }
}
