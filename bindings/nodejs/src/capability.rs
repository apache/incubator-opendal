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

/// Capability is used to describe what operations are supported
///  by the current Operator.
///
/// Via capability, we can know:
///
/// - Whether current Operator supports `read` or not.
/// - Whether current Operator supports `read` with if match or not.
/// - What's current Operator max supports batch operations count.
///
/// Add fields of Capabilities to be public and can be accessed directly.
#[napi]
pub struct Capability(opendal::Capability);

impl Capability {
    pub fn new(cap: opendal::Capability) -> Self {
        Self(cap)
    }
}

#[napi]
impl Capability {
    /// If operator supports stat.
    #[napi(getter)]
    pub fn stat(&self) -> bool {
        self.0.stat
    }

    /// If operator supports stat with if matched.
    #[napi(getter)]
    pub fn stat_with_if_match(&self) -> bool {
        self.0.stat_with_if_match
    }

    /// If operator supports stat with if not match.
    #[napi(getter)]
    pub fn stat_with_if_none_match(&self) -> bool {
        self.0.stat_with_if_none_match
    }

    /// If operator supports stat with if modified since.
    #[napi(getter)]
    pub fn stat_with_if_modified_since(&self) -> bool {
        self.0.stat_with_if_modified_since
    }

    /// If operator supports stat with if unmodified since.
    #[napi(getter)]
    pub fn stat_with_if_unmodified_since(&self) -> bool {
        self.0.stat_with_if_unmodified_since
    }

    /// If operator supports stat with versions.
    #[napi(getter)]
    pub fn stat_with_version(&self) -> bool {
        self.0.stat_with_version
    }

    /// If operator supports stat with override content type.
    #[napi(getter)]
    pub fn stat_with_override_content_type(&self) -> bool {
        self.0.stat_with_override_content_type
    }

    /// If operator supports stat with override cache control.
    #[napi(getter)]
    pub fn stat_with_override_cache_control(&self) -> bool {
        self.0.stat_with_override_cache_control
    }

    /// If operator supports stat with override content disposition.
    #[napi(getter)]
    pub fn stat_with_override_content_disposition(&self) -> bool {
        self.0.stat_with_override_content_disposition
    }

    /// If operator supports read.
    #[napi(getter)]
    pub fn read(&self) -> bool {
        self.0.read
    }

    /// If operator supports read with version.
    #[napi(getter)]
    pub fn read_with_version(&self) -> bool {
        self.0.read_with_version
    }

    /// If operator supports read with range.
    #[napi(getter)]
    pub fn read_with_if_modified_since(&self) -> bool {
        self.0.read_with_if_modified_since
    }

    /// If operator supports read with if unmodified since.
    #[napi(getter)]
    pub fn read_with_if_unmodified_since(&self) -> bool {
        self.0.read_with_if_unmodified_since
    }

    /// If operator supports read with if matched.
    #[napi(getter)]
    pub fn read_with_if_match(&self) -> bool {
        self.0.read_with_if_match
    }

    /// If operator supports read with if not match.
    #[napi(getter)]
    pub fn read_with_if_none_match(&self) -> bool {
        self.0.read_with_if_none_match
    }

    /// if operator supports read with override cache control.
    #[napi(getter)]
    pub fn read_with_override_cache_control(&self) -> bool {
        self.0.read_with_override_cache_control
    }

    /// if operator supports `read` with override content disposition.
    #[napi(getter)]
    pub fn read_with_override_content_disposition(&self) -> bool {
        self.0.read_with_override_content_disposition
    }

    /// if operator supports read with override content type.
    #[napi(getter)]
    pub fn read_with_override_content_type(&self) -> bool {
        self.0.read_with_override_content_type
    }

    /// If operator supports write.
    #[napi(getter)]
    pub fn write(&self) -> bool {
        self.0.write
    }

    /// If operator supports write can be called in multi times.
    #[napi(getter)]
    pub fn write_can_multi(&self) -> bool {
        self.0.write_can_multi
    }

    /// If operator supports write with empty content.
    #[napi(getter)]
    pub fn write_can_empty(&self) -> bool {
        self.0.write_can_empty
    }

    /// If operator supports write by append.
    #[napi(getter)]
    pub fn write_can_append(&self) -> bool {
        self.0.write_can_append
    }

    /// If operator supports write with content type.
    #[napi(getter)]
    pub fn write_with_content_type(&self) -> bool {
        self.0.write_with_content_type
    }

    /// If operator supports write with content disposition.
    #[napi(getter)]
    pub fn write_with_content_disposition(&self) -> bool {
        self.0.write_with_content_disposition
    }

    /// If operator supports write with cache control.
    #[napi(getter)]
    pub fn write_with_cache_control(&self) -> bool {
        self.0.write_with_cache_control
    }

    /// If operator supports write with content encoding.
    #[napi(getter)]
    pub fn write_with_content_encoding(&self) -> bool {
        self.0.write_with_content_encoding
    }

    /// If operator supports write with user metadata.
    #[napi(getter)]
    pub fn write_with_user_metadata(&self) -> bool {
        self.0.write_with_user_metadata
    }

    /// If operator supports write with if match.
    #[napi(getter)]
    pub fn write_with_if_match(&self) -> bool {
        self.0.write_with_if_match
    }

    /// If operator supports write with if none match.
    #[napi(getter)]
    pub fn write_with_if_none_match(&self) -> bool {
        self.0.write_with_if_none_match
    }

    /// If operator supports write with if not exists.
    #[napi(getter)]
    pub fn write_with_if_not_exists(&self) -> bool {
        self.0.write_with_if_not_exists
    }

    /// write_multi_max_size is the max size that services support in write_multi.
    ///
    /// For example, AWS S3 supports 5GiB as max in write_multi.
    #[napi(getter)]
    pub fn write_multi_max_size(&self) -> Option<usize> {
        self.0.write_multi_max_size
    }

    /// write_multi_min_size is the min size that services support in write_multi.
    ///
    /// For example, AWS S3 requires at least 5MiB in write_multi expect the last one.
    #[napi(getter)]
    pub fn write_multi_min_size(&self) -> Option<usize> {
        self.0.write_multi_min_size
    }
    /// write_total_max_size is the max size that services support in write_total.
    ///
    /// For example, Cloudflare D1 supports 1MB as max in write_total.
    #[napi(getter)]
    pub fn write_total_max_size(&self) -> Option<usize> {
        self.0.write_total_max_size
    }

    /// If operator supports create dir.
    #[napi(getter)]
    pub fn create_dir(&self) -> bool {
        self.0.create_dir
    }

    /// If operator supports delete.
    #[napi(getter)]
    pub fn delete(&self) -> bool {
        self.0.delete
    }

    /// If operator supports delete by version.
    #[napi(getter)]
    pub fn delete_with_version(&self) -> bool {
        self.0.delete_with_version
    }

    /// If operator supports copy.
    #[napi(getter)]
    pub fn copy(&self) -> bool {
        self.0.copy
    }

    /// If operator supports rename.
    #[napi(getter)]
    pub fn rename(&self) -> bool {
        self.0.rename
    }

    /// If operator supports list.
    #[napi(getter)]
    pub fn list(&self) -> bool {
        self.0.list
    }

    /// If backend supports list with limit.
    #[napi(getter)]
    pub fn list_with_limit(&self) -> bool {
        self.0.list_with_limit
    }

    /// If backend supports list with start after.
    #[napi(getter)]
    pub fn list_with_start_after(&self) -> bool {
        self.0.list_with_start_after
    }

    /// If backend supports list with recursive.
    #[napi(getter)]
    pub fn list_with_recursive(&self) -> bool {
        self.0.list_with_recursive
    }

    /// If backend supports list with versions.
    #[napi(getter)]
    pub fn list_with_versions(&self) -> bool {
        self.0.list_with_versions
    }

    /// If backend supports list with deleted.
    #[napi(getter)]
    pub fn list_with_deleted(&self) -> bool {
        self.0.list_with_deleted
    }

    /// If operator supports presign.
    #[napi(getter)]
    pub fn presign(&self) -> bool {
        self.0.presign
    }

    /// If operator supports presign read.
    #[napi(getter)]
    pub fn presign_read(&self) -> bool {
        self.0.presign_read
    }

    /// If operator supports presign stat.
    #[napi(getter)]
    pub fn presign_stat(&self) -> bool {
        self.0.presign_stat
    }

    /// If operator supports presign write.
    #[napi(getter)]
    pub fn presign_write(&self) -> bool {
        self.0.presign_write
    }

    /// If operator supports presign delete.
    #[napi(getter)]
    pub fn presign_delete(&self) -> bool {
        self.0.presign_delete
    }

    /// If operator supports shared.
    #[napi(getter)]
    pub fn shared(&self) -> bool {
        self.0.shared
    }
}
