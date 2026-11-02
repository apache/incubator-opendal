# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

from typing import final

@final
class Capability:
    """
    Capability defines the supported operations and their constraints for an Operator.

    This structure provides a comprehensive description of an Operator's
    capabilities, including:

    - Basic operations support (read, write, delete, etc.)
    - Advanced operation variants (conditional operations, metadata handling)
    - Operational constraints (size limits, batch limitations)
    """

    @property
    def copy(self, /) -> bool:
        """If operator supports copy."""
    @property
    def create_dir(self, /) -> bool:
        """If operator supports create dir."""
    @property
    def delete(self, /) -> bool:
        """If operator supports delete."""
    @property
    def delete_with_recursive(self, /) -> bool:
        """If recursive delete operations are supported."""
    @property
    def delete_with_version(self, /) -> bool:
        """If delete operations with version are supported."""
    @property
    def list(self, /) -> bool:
        """If operator supports list."""
    @property
    def list_with_deleted(self, /) -> bool:
        """If backend supports list with deleted."""
    @property
    def list_with_limit(self, /) -> bool:
        """If backend supports list with limit."""
    @property
    def list_with_recursive(self, /) -> bool:
        """If backend supports list without delimiter."""
    @property
    def list_with_start_after(self, /) -> bool:
        """If backend supports list with start after."""
    @property
    def list_with_versions(self, /) -> bool:
        """If backend supports list with versions."""
    @property
    def presign(self, /) -> bool:
        """If operator supports presign."""
    @property
    def presign_delete(self, /) -> bool:
        """If operator supports presign delete."""
    @property
    def presign_read(self, /) -> bool:
        """If operator supports presign read."""
    @property
    def presign_stat(self, /) -> bool:
        """If operator supports presign stat."""
    @property
    def presign_write(self, /) -> bool:
        """If operator supports presign write."""
    @property
    def read(self, /) -> bool:
        """If the operator supports read operations."""
    @property
    def read_with_if_match(self, /) -> bool:
        """If conditional read operations using If-Match are supported."""
    @property
    def read_with_if_modified_since(self, /) -> bool:
        """If conditional read operations using If-Modified-Since are supported."""
    @property
    def read_with_if_none_match(self, /) -> bool:
        """If conditional read operations using If-None-Match are supported."""
    @property
    def read_with_if_unmodified_since(self, /) -> bool:
        """If conditional read operations using If-Unmodified-Since are supported."""
    @property
    def read_with_override_cache_control(self, /) -> bool:
        """If Cache-Control header override is supported during read operations."""
    @property
    def read_with_override_content_disposition(self, /) -> bool:
        """If Content-Disposition header can be overridden during read operations."""
    @property
    def read_with_override_content_type(self, /) -> bool:
        """If Content-Type header override is supported during read operations."""
    @property
    def read_with_version(self, /) -> bool:
        """If versions read operations are supported."""
    @property
    def rename(self, /) -> bool:
        """If operator supports rename."""
    @property
    def shared(self, /) -> bool:
        """If operator supports shared."""
    @property
    def stat(self, /) -> bool:
        """If operator supports stat."""
    @property
    def stat_with_if_match(self, /) -> bool:
        """If operator supports stat with if match."""
    @property
    def stat_with_if_modified_since(self, /) -> bool:
        """If operator supports stat with if modified since."""
    @property
    def stat_with_if_none_match(self, /) -> bool:
        """If operator supports stat with if none match."""
    @property
    def stat_with_if_unmodified_since(self, /) -> bool:
        """If operator supports stat with if unmodified since."""
    @property
    def stat_with_version(self, /) -> bool:
        """If operator supports stat with version."""
    @property
    def write(self, /) -> bool:
        """If the operator supports write operations."""
    @property
    def write_can_append(self, /) -> bool:
        """If append operations are supported."""
    @property
    def write_can_empty(self, /) -> bool:
        """If writing empty content is supported."""
    @property
    def write_can_multi(self, /) -> bool:
        """If multiple write operations can be performed on the same object."""
    @property
    def write_multi_max_size(self, /) -> int | None:
        """
        Maximum size supported for multipart uploads.

        For example, AWS S3 supports up to 5GiB per part in multipart uploads.
        """
    @property
    def write_multi_min_size(self, /) -> int | None:
        """
        Minimum size required for multipart uploads (except for the last part).

        For example, AWS S3 requires at least 5MiB per part.
        """
    @property
    def write_total_max_size(self, /) -> int | None:
        """
        Maximum total size supported for write operations.

        For example, Cloudflare D1 has a 1MB total size limit.
        """
    @property
    def write_with_cache_control(self, /) -> bool:
        """If Cache-Control can be specified during write operations."""
    @property
    def write_with_content_disposition(self, /) -> bool:
        """If Content-Disposition can be specified during write operations."""
    @property
    def write_with_content_encoding(self, /) -> bool:
        """If Content-Encoding can be specified during write operations."""
    @property
    def write_with_content_type(self, /) -> bool:
        """If Content-Type can be specified during write operations."""
    @property
    def write_with_if_match(self, /) -> bool:
        """If conditional write operations using If-Match are supported."""
    @property
    def write_with_if_none_match(self, /) -> bool:
        """If conditional write operations using If-None-Match are supported."""
    @property
    def write_with_if_not_exists(self, /) -> bool:
        """If write operations can be conditional on object non-existence."""
    @property
    def write_with_user_metadata(self, /) -> bool:
        """If custom user metadata can be attached during write operations."""
