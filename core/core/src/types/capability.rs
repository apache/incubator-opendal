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

use std::fmt::Debug;

use serde::Deserialize;
use serde::Serialize;

/// Capability defines the supported operations and their constraints for a storage Operator.
///
/// # Overview
///
/// This structure provides a comprehensive description of an Operator's capabilities,
/// including:
///
/// - Basic operations support (read, write, delete, etc.)
/// - Advanced operation variants (conditional operations, metadata handling)
/// - Operational constraints (size limits, batch limitations)
///
/// # Capability Type
///
/// [`OperatorInfo::capability`][crate::OperatorInfo::capability] represents all
/// operations available on the current operator, including those implemented
/// through layers.
///
/// # Implementation Details
///
/// Some operations might be available even when not natively supported by the
/// backend. For example:
///
/// - Blocking operations are provided through the BlockingLayer
///
/// Developers should use `capability` to determine available operations.
/// Conditional capability fields follow the
/// [conditional operation specification][crate::docs::specs::conditional_operations].
///
/// # Field Naming Conventions
///
/// Fields follow these naming patterns:
///
/// - Basic operations: Simple lowercase (e.g., `read`, `write`)
/// - Compound operations: Underscore-separated (e.g., `presign_read`)
/// - Variants: Capability description (e.g., `write_can_empty`)
/// - Parameterized operations: With-style (e.g., `read_with_if_match`)
/// - Limitations: Constraint description (e.g., `write_multi_max_size`)
/// - Metadata Results: Returning metadata capabilities (e.g., `stat_has_content_length`)
///
/// All capability fields are public and can be accessed directly.
#[derive(Copy, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    /// Indicates if the operator supports metadata retrieval operations.
    pub stat: bool,
    /// Indicates if conditional stat operations using If-Match are supported.
    pub stat_with_if_match: bool,
    /// Indicates if conditional stat operations using If-None-Match are supported.
    pub stat_with_if_none_match: bool,
    /// Indicates if conditional stat operations using version match are supported.
    pub stat_with_if_version_match: bool,
    /// Indicates if conditional stat operations using version non-match are supported.
    pub stat_with_if_version_not_match: bool,
    /// Indicates if conditional stat operations using If-Modified-Since are supported.
    pub stat_with_if_modified_since: bool,
    /// Indicates if conditional stat operations using If-Unmodified-Since are supported.
    pub stat_with_if_unmodified_since: bool,
    /// Indicates if Cache-Control header override is supported during stat operations.
    pub stat_with_override_cache_control: bool,
    /// Indicates if Content-Disposition header override is supported during stat operations.
    pub stat_with_override_content_disposition: bool,
    /// Indicates if Content-Type header override is supported during stat operations.
    pub stat_with_override_content_type: bool,
    /// Indicates if versions stat operations are supported.
    pub stat_with_version: bool,

    /// Indicates if the operator supports read operations.
    pub read: bool,
    /// Indicates if conditional read operations using If-Match are supported.
    pub read_with_if_match: bool,
    /// Indicates if conditional read operations using If-None-Match are supported.
    pub read_with_if_none_match: bool,
    /// Indicates if conditional read operations using version match are supported.
    pub read_with_if_version_match: bool,
    /// Indicates if conditional read operations using version non-match are supported.
    pub read_with_if_version_not_match: bool,
    /// Indicates if conditional read operations using If-Modified-Since are supported.
    pub read_with_if_modified_since: bool,
    /// Indicates if conditional read operations using If-Unmodified-Since are supported.
    pub read_with_if_unmodified_since: bool,
    /// Indicates if Cache-Control header override is supported during read operations.
    pub read_with_override_cache_control: bool,
    /// Indicates if Content-Disposition header override is supported during read operations.
    pub read_with_override_content_disposition: bool,
    /// Indicates if Content-Type header override is supported during read operations.
    pub read_with_override_content_type: bool,
    /// Indicates if versions read operations are supported.
    pub read_with_version: bool,
    /// Indicates if suffix read operations are supported.
    pub read_with_suffix: bool,

    /// Indicates if the operator supports write operations.
    pub write: bool,
    /// Indicates if multiple write operations can be performed on the same object.
    pub write_can_multi: bool,
    /// Indicates if writing empty content is supported.
    pub write_can_empty: bool,
    /// Indicates if append operations are supported.
    pub write_can_append: bool,
    /// Indicates if a non-append writer can natively copy source ranges into an in-progress write.
    /// `Writer::copy_from` remains available through streaming fallback when false or when appending.
    pub write_can_copy_from: bool,
    /// Indicates if Content-Type can be specified during write operations.
    pub write_with_content_type: bool,
    /// Indicates if Content-Disposition can be specified during write operations.
    pub write_with_content_disposition: bool,
    /// Indicates if Content-Encoding can be specified during write operations.
    pub write_with_content_encoding: bool,
    /// Indicates if Cache-Control can be specified during write operations.
    pub write_with_cache_control: bool,
    /// Indicates if conditional write operations using If-Match are supported.
    pub write_with_if_match: bool,
    /// Indicates if conditional write operations using If-None-Match are supported.
    pub write_with_if_none_match: bool,
    /// Indicates if conditional write operations using version match are supported.
    pub write_with_if_version_match: bool,
    /// Indicates if conditional write operations using version non-match are supported.
    pub write_with_if_version_not_match: bool,
    /// Indicates if write operations can be conditional on object non-existence.
    pub write_with_if_not_exists: bool,
    /// Indicates if custom user metadata can be attached during write operations.
    pub write_with_user_metadata: bool,
    /// Maximum size supported for multipart uploads.
    /// For example, AWS S3 supports up to 5GiB per part in multipart uploads.
    pub write_multi_max_size: Option<usize>,
    /// Minimum size required for multipart uploads (except for the last part).
    /// For example, AWS S3 requires at least 5MiB per part.
    pub write_multi_min_size: Option<usize>,
    /// Maximum total size supported for write operations.
    /// For example, Cloudflare D1 has a 1MB total size limit.
    pub write_total_max_size: Option<usize>,

    /// Indicates if directory creation is supported.
    pub create_dir: bool,

    /// Indicates if delete operations are supported.
    pub delete: bool,
    /// Indicates if versions delete operations are supported.
    pub delete_with_version: bool,
    /// Indicates if recursive delete operations are supported.
    pub delete_with_recursive: bool,
    /// Indicates if conditional delete operations using If-Match are supported.
    pub delete_with_if_match: bool,
    /// Indicates if conditional delete operations using If-None-Match are supported.
    pub delete_with_if_none_match: bool,
    /// Indicates if conditional delete operations using version match are supported.
    pub delete_with_if_version_match: bool,
    /// Indicates if conditional delete operations using version non-match are supported.
    pub delete_with_if_version_not_match: bool,
    /// Maximum size supported for single delete operations.
    pub delete_max_size: Option<usize>,

    /// Indicates if copy operations are supported.
    pub copy: bool,
    /// Indicates if conditional copy operations with if-not-exists are supported.
    pub copy_with_if_not_exists: bool,
    /// Indicates if conditional copy operations with if-match are supported.
    pub copy_with_if_match: bool,
    /// Indicates if conditional copy operations with if-none-match are supported.
    pub copy_with_if_none_match: bool,
    /// Indicates if conditional copy operations using version match are supported.
    pub copy_with_if_version_match: bool,
    /// Indicates if conditional copy operations using version non-match are supported.
    pub copy_with_if_version_not_match: bool,
    /// Indicates if copy operations from a specific source version are supported.
    pub copy_with_source_version: bool,
    /// Indicates if copy operations can be split into multiple server-side tasks.
    pub copy_can_multi: bool,
    /// Maximum size supported for segmented copy tasks.
    pub copy_multi_max_size: Option<usize>,
    /// Minimum size required for segmented copy tasks.
    pub copy_multi_min_size: Option<usize>,

    /// Indicates if the operator supports composing complete source objects.
    pub compose: bool,
    /// Indicates if Content-Type can be specified during composition.
    pub compose_with_content_type: bool,
    /// Indicates if Content-Disposition can be specified during composition.
    pub compose_with_content_disposition: bool,
    /// Indicates if Content-Encoding can be specified during composition.
    pub compose_with_content_encoding: bool,
    /// Indicates if Cache-Control can be specified during composition.
    pub compose_with_cache_control: bool,
    /// Indicates if custom user metadata can be attached during composition.
    pub compose_with_user_metadata: bool,
    /// Indicates if composition supports a destination ETag match condition.
    pub compose_with_if_match: bool,
    /// Indicates if composition supports a destination ETag non-match condition.
    pub compose_with_if_none_match: bool,
    /// Indicates if composition supports a destination version match condition.
    pub compose_with_if_version_match: bool,
    /// Indicates if composition supports a destination version non-match condition.
    pub compose_with_if_version_not_match: bool,
    /// Indicates if composition can require a missing destination.
    pub compose_with_if_not_exists: bool,
    /// Indicates if composition can select a specific source version.
    pub compose_with_source_version: bool,
    /// Indicates if composition supports a source ETag match condition.
    pub compose_with_source_if_match: bool,

    /// Indicates if restore operations are supported.
    pub restore: bool,
    /// Indicates if restoring a specific version is supported.
    pub restore_with_version: bool,
    /// Indicates if conditional restore operations using if-not-exists are supported.
    pub restore_with_if_not_exists: bool,

    /// Indicates if rename operations are supported.
    pub rename: bool,
    /// Indicates if conditional rename operations with if-not-exists are supported.
    pub rename_with_if_not_exists: bool,

    /// Indicates if list operations are supported.
    pub list: bool,
    /// Indicates if list operations support result limiting.
    pub list_with_limit: bool,
    /// Indicates if list operations support continuation from a specific point.
    pub list_with_start_after: bool,
    /// Indicates if recursive listing is supported.
    pub list_with_recursive: bool,
    /// Indicates if listing with versions included is supported.
    pub list_with_versions: bool,
    /// Indicates if listing with deleted files included is supported.
    pub list_with_deleted: bool,

    /// Indicates if presigned URL generation is supported.
    pub presign: bool,
    /// Indicates if presigned URLs for read operations are supported.
    pub presign_read: bool,
    /// Indicates if presigned URLs for stat operations are supported.
    pub presign_stat: bool,
    /// Indicates if presigned URLs for write operations are supported.
    pub presign_write: bool,
    /// Indicates if presigned URLs for delete operations are supported.
    pub presign_delete: bool,

    /// Indicate if the operator supports shared access.
    pub shared: bool,
}

impl Debug for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // NOTE: All services in opendal are readable.
        if self.read {
            f.write_str("Read")?;
        }
        if self.write {
            f.write_str("| Write")?;
        }
        if self.list {
            f.write_str("| List")?;
        }
        if self.presign {
            f.write_str("| Presign")?;
        }
        if self.shared {
            f.write_str("| Shared")?;
        }

        Ok(())
    }
}
