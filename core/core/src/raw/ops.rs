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

//! Ops provides the operation args struct like [`OpRead`] for user.
//!
//! By using ops, users can add more context for operation.

use crate::BytesRange;
use crate::UserMetadata;
use crate::options;
use crate::raw::*;
use crate::types::compact::CompactValues;
use crate::types::metadata::user_metadata_encoded_len;
use crate::types::metadata::write_user_metadata;
use crate::{Capability, Error, ErrorKind, Result};

use std::collections::HashMap;

#[inline]
fn string_value(values: &CompactValues, field: usize) -> Option<&str> {
    values.get_str(field)
}

#[inline]
fn encode_timestamp(value: Timestamp) -> [u8; 12] {
    let value = value.into_inner();
    let mut encoded = [0; 12];
    encoded[..8].copy_from_slice(&value.as_second().to_le_bytes());
    encoded[8..].copy_from_slice(&value.subsec_nanosecond().to_le_bytes());
    encoded
}

#[inline]
fn decode_timestamp(value: &[u8]) -> Timestamp {
    let seconds = i64::from_le_bytes(value[..8].try_into().unwrap());
    let nanoseconds = i32::from_le_bytes(value[8..].try_into().unwrap());
    Timestamp::new(seconds, nanoseconds).expect("operation stores a previously validated timestamp")
}

fn sorted_user_metadata(value: HashMap<String, String>) -> Vec<(String, String)> {
    let mut value: Vec<_> = value.into_iter().collect();
    value.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    value
}

/// Arguments for `create` operation.
///
/// The path must be normalized.
#[derive(Debug, Clone, Default)]
pub struct OpCreateDir {}

impl OpCreateDir {
    /// Create a new `OpCreateDir`.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Arguments for `delete` operation.
///
/// The path must be normalized.
#[derive(Debug, Clone, Default, Eq, Hash, PartialEq)]
pub struct OpDelete {
    flags: u8,
    values: CompactValues,
}

const OP_DELETE_RECURSIVE: u8 = 1;

#[repr(usize)]
enum DeleteField {
    Version,
    IfMatch,
    IfNoneMatch,
    IfVersionMatch,
    IfVersionNotMatch,
}

impl OpDelete {
    /// Create a new `OpDelete`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lower `options` against `capability` and freeze them into service-ready arguments.
    ///
    /// # Errors
    ///
    /// Returns an error when `if_not_changed` contains no identity or conflicts
    /// with an explicit equality condition.
    ///
    /// # Panics
    ///
    /// Panics when the compact value block, including its index, exceeds `u16::MAX` bytes.
    pub fn from_options(
        capability: &Capability,
        mut options: options::DeleteOptions,
    ) -> Result<Self> {
        if let Some(metadata) = options.if_not_changed.take() {
            let (target, identity, name) = if capability.delete_with_if_version_match
                && let Some(version) = metadata.version()
            {
                (&mut options.if_version_match, version, "if_version_match")
            } else if let Some(etag) = metadata.etag() {
                (&mut options.if_match, etag, "if_match")
            } else if let Some(version) = metadata.version() {
                (&mut options.if_version_match, version, "if_version_match")
            } else {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "if_not_changed metadata contains neither version nor ETag",
                )
                .with_operation(Operation::Delete));
            };

            if let Some(explicit) = target {
                if explicit != identity {
                    return Err(Error::new(
                        ErrorKind::ConditionNotMatch,
                        format!("if_not_changed conflicts with {name}"),
                    )
                    .with_operation(Operation::Delete));
                }
            } else {
                *target = Some(identity.to_owned());
            }
        }

        let fields = [
            options.version.as_deref().map(str::as_bytes),
            options.if_match.as_deref().map(str::as_bytes),
            options.if_none_match.as_deref().map(str::as_bytes),
            options.if_version_match.as_deref().map(str::as_bytes),
            options.if_version_not_match.as_deref().map(str::as_bytes),
        ];
        Ok(Self {
            flags: if options.recursive {
                OP_DELETE_RECURSIVE
            } else {
                0
            },
            values: CompactValues::encode(&fields),
        })
    }
}

impl OpDelete {
    /// Return the version of the object to delete.
    #[inline]
    pub fn version(&self) -> Option<&str> {
        string_value(&self.values, DeleteField::Version as usize)
    }

    /// Whether this delete should remove objects recursively.
    #[inline]
    pub fn recursive(&self) -> bool {
        self.flags & OP_DELETE_RECURSIVE != 0
    }

    /// Return the ETag that the object must match before deletion.
    #[inline]
    pub fn if_match(&self) -> Option<&str> {
        string_value(&self.values, DeleteField::IfMatch as usize)
    }

    /// Return the ETag that the object must not match before deletion.
    #[inline]
    pub fn if_none_match(&self) -> Option<&str> {
        string_value(&self.values, DeleteField::IfNoneMatch as usize)
    }

    /// Return the version that the current object must match before deletion.
    #[inline]
    pub fn if_version_match(&self) -> Option<&str> {
        string_value(&self.values, DeleteField::IfVersionMatch as usize)
    }

    /// Return the version that the current object must not match before deletion.
    #[inline]
    pub fn if_version_not_match(&self) -> Option<&str> {
        string_value(&self.values, DeleteField::IfVersionNotMatch as usize)
    }

    /// Returns whether at least one value-bearing conditional option is set.
    #[inline]
    pub fn is_conditional(&self) -> bool {
        self.if_match().is_some()
            || self.if_none_match().is_some()
            || self.if_version_match().is_some()
            || self.if_version_not_match().is_some()
    }

    pub(crate) fn set_recursive(&mut self, recursive: bool) {
        if recursive {
            self.flags |= OP_DELETE_RECURSIVE;
        } else {
            self.flags &= !OP_DELETE_RECURSIVE;
        }
    }

    #[doc(hidden)]
    pub fn into_version(mut self, version: &str) -> Self {
        self.values = self
            .values
            .replace(DeleteField::Version as usize, version.as_bytes());
        self
    }
}

/// Arguments for `delete` operation.
///
/// The path must be normalized.
#[derive(Debug, Clone, Default)]
pub struct OpDeleter {}

impl OpDeleter {
    /// Create a new `OpDelete`.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Arguments for `list` operation.
#[derive(Debug, Clone, Default)]
pub struct OpList {
    /// The maximum number of results that the service should return per request.
    ///
    /// This can be used to control the memory consumption of a list operation.
    limit: usize,
    flags: u8,
    values: CompactValues,
}

const OP_LIST_HAS_LIMIT: u8 = 1;
const OP_LIST_RECURSIVE: u8 = 1 << 1;
const OP_LIST_VERSIONS: u8 = 1 << 2;
const OP_LIST_DELETED: u8 = 1 << 3;

impl OpList {
    /// Create a new `OpList`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the maximum number of results per request.
    #[inline]
    pub fn limit(&self) -> Option<usize> {
        (self.flags & OP_LIST_HAS_LIMIT != 0).then_some(self.limit)
    }

    /// Return the key after which listing should start.
    #[inline]
    pub fn start_after(&self) -> Option<&str> {
        string_value(&self.values, 0)
    }

    /// Return whether the list operation is recursive.
    #[inline]
    pub fn recursive(&self) -> bool {
        self.flags & OP_LIST_RECURSIVE != 0
    }

    /// Get the concurrent of list operation.
    #[deprecated(since = "0.53.2", note = "concurrent in list is no-op")]
    #[inline]
    pub fn concurrent(&self) -> usize {
        0
    }

    /// Return whether the operation includes object versions.
    #[inline]
    pub fn versions(&self) -> bool {
        self.flags & OP_LIST_VERSIONS != 0
    }

    /// Return whether the operation includes deleted objects.
    #[inline]
    pub fn deleted(&self) -> bool {
        self.flags & OP_LIST_DELETED != 0
    }
}

impl From<options::ListOptions> for OpList {
    fn from(value: options::ListOptions) -> Self {
        let mut flags = 0;
        if value.limit.is_some() {
            flags |= OP_LIST_HAS_LIMIT;
        }
        if value.recursive {
            flags |= OP_LIST_RECURSIVE;
        }
        if value.versions {
            flags |= OP_LIST_VERSIONS;
        }
        if value.deleted {
            flags |= OP_LIST_DELETED;
        }
        Self {
            limit: value.limit.unwrap_or_default(),
            flags,
            values: CompactValues::encode(&[value.start_after.as_deref().map(str::as_bytes)]),
        }
    }
}

/// Arguments for `presign` operation.
///
/// The path must be normalized.
#[derive(Debug, Clone)]
pub struct OpPresign {
    expire: Duration,

    op: PresignOperation,
}

impl OpPresign {
    /// Create a new `OpPresign`.
    pub fn new(op: impl Into<PresignOperation>, expire: Duration) -> Self {
        Self {
            op: op.into(),
            expire,
        }
    }

    /// Return the operation to presign.
    pub fn operation(&self) -> &PresignOperation {
        &self.op
    }

    /// Return the request expiration duration.
    pub fn expire(&self) -> Duration {
        self.expire
    }

    /// Consume OpPresign into (Duration, PresignOperation)
    pub fn into_parts(self) -> (Duration, PresignOperation) {
        (self.expire, self.op)
    }
}

/// Presign operation used for presign.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PresignOperation {
    /// Presign a stat(head) operation.
    Stat(OpStat),
    /// Presign a read operation.
    Read(BytesRange, OpRead),
    /// Presign a write operation.
    Write(OpWrite),
    /// Presign a delete operation.
    Delete(OpDelete),
}

impl From<OpStat> for PresignOperation {
    fn from(op: OpStat) -> Self {
        Self::Stat(op)
    }
}

impl From<OpRead> for PresignOperation {
    fn from(v: OpRead) -> Self {
        Self::Read(BytesRange::default(), v)
    }
}

impl From<OpWrite> for PresignOperation {
    fn from(v: OpWrite) -> Self {
        Self::Write(v)
    }
}

impl From<OpDelete> for PresignOperation {
    fn from(v: OpDelete) -> Self {
        Self::Delete(v)
    }
}

/// Arguments for `read` operation.
#[derive(Debug, Clone, Default)]
pub struct OpRead {
    values: CompactValues,
}

#[repr(usize)]
enum ReadField {
    IfMatch,
    IfNoneMatch,
    IfVersionMatch,
    IfVersionNotMatch,
    IfModifiedSince,
    IfUnmodifiedSince,
    OverrideContentType,
    OverrideCacheControl,
    OverrideContentDisposition,
    Version,
    ContentLengthHint,
}

impl OpRead {
    /// Create a default `OpRead` which will read whole content of path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lower compose source `options` against `capability` and freeze them into
    /// service-ready read arguments.
    ///
    /// # Errors
    ///
    /// Returns an error when `if_not_changed` contains no identity or conflicts
    /// with an explicit source equality condition.
    ///
    /// # Panics
    ///
    /// Panics when the compact value block, including its index, exceeds `u16::MAX` bytes.
    pub(crate) fn from_compose_source_options(
        capability: &Capability,
        mut options: options::ComposeSourceOptions,
    ) -> Result<Self> {
        if let Some(metadata) = options.if_not_changed.take() {
            let (target, identity, name) = if capability.compose_with_source_version
                && let Some(version) = metadata.version()
            {
                (&mut options.version, version, "source version")
            } else if let Some(etag) = metadata.etag() {
                (&mut options.if_match, etag, "source if_match")
            } else if let Some(version) = metadata.version() {
                (&mut options.version, version, "source version")
            } else {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "if_not_changed metadata contains neither version nor ETag",
                )
                .with_operation(Operation::Compose));
            };

            if let Some(explicit) = target {
                if explicit != identity {
                    return Err(Error::new(
                        ErrorKind::ConditionNotMatch,
                        format!("if_not_changed conflicts with {name}"),
                    )
                    .with_operation(Operation::Compose));
                }
            } else {
                *target = Some(identity.to_owned());
            }
        }

        Ok(Self::from_read_options(options::ReadOptions {
            version: options.version,
            if_match: options.if_match,
            ..Default::default()
        }))
    }

    /// Returns the content-disposition header that should be sent back by the remote read
    /// operation.
    #[inline]
    pub fn override_content_disposition(&self) -> Option<&str> {
        string_value(&self.values, ReadField::OverrideContentDisposition as usize)
    }

    /// Returns the cache-control header that should be sent back by the remote read operation.
    #[inline]
    pub fn override_cache_control(&self) -> Option<&str> {
        string_value(&self.values, ReadField::OverrideCacheControl as usize)
    }

    /// Returns the content-type header that should be sent back by the remote read operation.
    #[inline]
    pub fn override_content_type(&self) -> Option<&str> {
        string_value(&self.values, ReadField::OverrideContentType as usize)
    }

    /// Get If-Match from option
    #[inline]
    pub fn if_match(&self) -> Option<&str> {
        string_value(&self.values, ReadField::IfMatch as usize)
    }

    /// Get If-None-Match from option
    #[inline]
    pub fn if_none_match(&self) -> Option<&str> {
        string_value(&self.values, ReadField::IfNoneMatch as usize)
    }

    /// Get the version match condition.
    #[inline]
    pub fn if_version_match(&self) -> Option<&str> {
        string_value(&self.values, ReadField::IfVersionMatch as usize)
    }

    /// Get the version non-match condition.
    #[inline]
    pub fn if_version_not_match(&self) -> Option<&str> {
        string_value(&self.values, ReadField::IfVersionNotMatch as usize)
    }

    /// Return the If-Modified-Since condition.
    #[inline]
    pub fn if_modified_since(&self) -> Option<Timestamp> {
        self.values
            .get(ReadField::IfModifiedSince as usize)
            .map(decode_timestamp)
    }

    /// Get If-Unmodified-Since from option
    #[inline]
    pub fn if_unmodified_since(&self) -> Option<Timestamp> {
        self.values
            .get(ReadField::IfUnmodifiedSince as usize)
            .map(decode_timestamp)
    }

    /// Returns whether at least one value-bearing conditional option is set.
    #[inline]
    pub fn is_conditional(&self) -> bool {
        self.if_match().is_some()
            || self.if_none_match().is_some()
            || self.if_version_match().is_some()
            || self.if_version_not_match().is_some()
            || self.if_modified_since().is_some()
            || self.if_unmodified_since().is_some()
    }

    /// Get version from option
    #[inline]
    pub fn version(&self) -> Option<&str> {
        string_value(&self.values, ReadField::Version as usize)
    }

    pub(crate) fn content_length_hint(&self) -> Option<u64> {
        self.values
            .get(ReadField::ContentLengthHint as usize)
            .map(|value| u64::from_le_bytes(value.try_into().unwrap()))
    }

    fn from_read_options(value: options::ReadOptions) -> Self {
        let if_modified_since = value.if_modified_since.map(encode_timestamp);
        let if_unmodified_since = value.if_unmodified_since.map(encode_timestamp);
        let content_length_hint = value.content_length_hint.map(u64::to_le_bytes);
        Self {
            values: CompactValues::encode(&[
                value.if_match.as_deref().map(str::as_bytes),
                value.if_none_match.as_deref().map(str::as_bytes),
                value.if_version_match.as_deref().map(str::as_bytes),
                value.if_version_not_match.as_deref().map(str::as_bytes),
                if_modified_since.as_ref().map(|value| value.as_slice()),
                if_unmodified_since.as_ref().map(|value| value.as_slice()),
                value.override_content_type.as_deref().map(str::as_bytes),
                value.override_cache_control.as_deref().map(str::as_bytes),
                value
                    .override_content_disposition
                    .as_deref()
                    .map(str::as_bytes),
                value.version.as_deref().map(str::as_bytes),
                content_length_hint.as_ref().map(|value| value.as_slice()),
            ]),
        }
    }
}

/// Arguments for reader operation.
#[derive(Debug, Clone)]
pub struct OpReader {
    /// The number of concurrent requests that reader can send.
    concurrent: usize,
    /// Request chunk size.
    chunk: Option<usize>,
    /// The gap size of each request.
    gap: Option<usize>,
    /// The maximum number of buffers that can be prefetched.
    prefetch: usize,
}

impl Default for OpReader {
    fn default() -> Self {
        Self {
            concurrent: 1,
            chunk: None,
            gap: None,
            prefetch: 0,
        }
    }
}

impl OpReader {
    /// Create a new `OpReader`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of concurrent requests the reader can send.
    pub fn with_concurrent(mut self, concurrent: usize) -> Self {
        self.concurrent = concurrent.max(1);
        self
    }

    /// Return the number of concurrent requests.
    pub fn concurrent(&self) -> usize {
        self.concurrent
    }

    /// Set the request chunk size.
    pub fn with_chunk(mut self, chunk: usize) -> Self {
        self.chunk = Some(chunk.max(1));
        self
    }

    /// Return the request chunk size.
    pub fn chunk(&self) -> Option<usize> {
        self.chunk
    }

    /// Set the gap size.
    ///
    /// Set to `0` to disable merging ranges separated by a gap. Overlapping or
    /// adjacent ranges are still merged.
    pub fn with_gap(mut self, gap: usize) -> Self {
        self.gap = Some(gap);
        self
    }

    /// Return the gap size.
    pub fn gap(&self) -> Option<usize> {
        self.gap
    }

    /// Set the number of prefetch requests.
    pub fn with_prefetch(mut self, prefetch: usize) -> Self {
        self.prefetch = prefetch;
        self
    }

    /// Return the number of prefetch requests.
    pub fn prefetch(&self) -> usize {
        self.prefetch
    }
}

impl From<options::ReadOptions> for (BytesRange, OpRead, OpReader) {
    fn from(value: options::ReadOptions) -> Self {
        let range = value.range;
        let reader = OpReader {
            concurrent: value.concurrent.max(1),
            chunk: value.chunk,
            gap: value.gap,
            prefetch: 0,
        };
        (range, OpRead::from_read_options(value), reader)
    }
}

impl From<options::ReaderOptions> for (OpRead, OpReader) {
    fn from(value: options::ReaderOptions) -> Self {
        let reader = OpReader {
            concurrent: value.concurrent.max(1),
            chunk: value.chunk,
            gap: value.gap,
            prefetch: value.prefetch,
        };
        let read = options::ReadOptions {
            version: value.version,
            if_match: value.if_match,
            if_none_match: value.if_none_match,
            if_version_match: value.if_version_match,
            if_version_not_match: value.if_version_not_match,
            if_modified_since: value.if_modified_since,
            if_unmodified_since: value.if_unmodified_since,
            content_length_hint: value.content_length_hint,
            ..Default::default()
        };
        (OpRead::from_read_options(read), reader)
    }
}

/// Arguments for `stat` operation.
#[derive(Debug, Clone, Default)]
pub struct OpStat {
    values: CompactValues,
}

impl OpStat {
    /// Create a new `OpStat`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get If-Match from option
    #[inline]
    pub fn if_match(&self) -> Option<&str> {
        string_value(&self.values, ReadField::IfMatch as usize)
    }

    /// Get If-None-Match from option
    #[inline]
    pub fn if_none_match(&self) -> Option<&str> {
        string_value(&self.values, ReadField::IfNoneMatch as usize)
    }

    /// Get the version match condition.
    #[inline]
    pub fn if_version_match(&self) -> Option<&str> {
        string_value(&self.values, ReadField::IfVersionMatch as usize)
    }

    /// Get the version non-match condition.
    #[inline]
    pub fn if_version_not_match(&self) -> Option<&str> {
        string_value(&self.values, ReadField::IfVersionNotMatch as usize)
    }

    /// Get If-Modified-Since from option
    #[inline]
    pub fn if_modified_since(&self) -> Option<Timestamp> {
        self.values
            .get(ReadField::IfModifiedSince as usize)
            .map(decode_timestamp)
    }

    /// Get If-Unmodified-Since from option
    #[inline]
    pub fn if_unmodified_since(&self) -> Option<Timestamp> {
        self.values
            .get(ReadField::IfUnmodifiedSince as usize)
            .map(decode_timestamp)
    }

    /// Returns whether at least one value-bearing conditional option is set.
    #[inline]
    pub fn is_conditional(&self) -> bool {
        self.if_match().is_some()
            || self.if_none_match().is_some()
            || self.if_version_match().is_some()
            || self.if_version_not_match().is_some()
            || self.if_modified_since().is_some()
            || self.if_unmodified_since().is_some()
    }

    /// Returns the content-disposition header that should be sent back by the remote read
    /// operation.
    #[inline]
    pub fn override_content_disposition(&self) -> Option<&str> {
        string_value(&self.values, ReadField::OverrideContentDisposition as usize)
    }

    /// Returns the cache-control header that should be sent back by the remote read operation.
    #[inline]
    pub fn override_cache_control(&self) -> Option<&str> {
        string_value(&self.values, ReadField::OverrideCacheControl as usize)
    }

    /// Returns the content-type header that should be sent back by the remote read operation.
    #[inline]
    pub fn override_content_type(&self) -> Option<&str> {
        string_value(&self.values, ReadField::OverrideContentType as usize)
    }

    /// Get version from option
    #[inline]
    pub fn version(&self) -> Option<&str> {
        string_value(&self.values, ReadField::Version as usize)
    }

    pub(crate) fn from_read(value: &OpRead) -> Self {
        // `OpStat` uses the same `ReadField` layout and simply ignores the
        // read-only content-length hint.
        Self {
            values: value.values.clone(),
        }
    }
}

impl From<options::StatOptions> for OpStat {
    fn from(value: options::StatOptions) -> Self {
        let if_modified_since = value.if_modified_since.map(encode_timestamp);
        let if_unmodified_since = value.if_unmodified_since.map(encode_timestamp);
        Self {
            values: CompactValues::encode(&[
                value.if_match.as_deref().map(str::as_bytes),
                value.if_none_match.as_deref().map(str::as_bytes),
                value.if_version_match.as_deref().map(str::as_bytes),
                value.if_version_not_match.as_deref().map(str::as_bytes),
                if_modified_since.as_ref().map(|value| value.as_slice()),
                if_unmodified_since.as_ref().map(|value| value.as_slice()),
                value.override_content_type.as_deref().map(str::as_bytes),
                value.override_cache_control.as_deref().map(str::as_bytes),
                value
                    .override_content_disposition
                    .as_deref()
                    .map(str::as_bytes),
                value.version.as_deref().map(str::as_bytes),
            ]),
        }
    }
}

/// Arguments for `write` operation.
#[derive(Debug, Clone, Default)]
pub struct OpWrite {
    concurrent: usize,
    flags: u8,
    values: CompactValues,
}

const OP_WRITE_APPEND: u8 = 1;
const OP_WRITE_IF_NOT_EXISTS: u8 = 1 << 1;

#[repr(usize)]
enum WriteField {
    ContentType,
    ContentDisposition,
    ContentEncoding,
    CacheControl,
    IfMatch,
    IfNoneMatch,
    IfVersionMatch,
    IfVersionNotMatch,
    UserMetadata,
}

impl OpWrite {
    /// Create a new `OpWrite`.
    ///
    /// If input path is not a file path, an error will be returned.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lower `options` against `capability` and freeze them into service-ready arguments.
    ///
    /// # Errors
    ///
    /// Returns an error when `if_not_changed` contains no identity or conflicts
    /// with an explicit equality condition.
    ///
    /// # Panics
    ///
    /// Panics when the compact value block, including its index, exceeds `u16::MAX` bytes.
    pub fn from_options(
        capability: &Capability,
        mut options: options::WriteOptions,
    ) -> Result<(Self, OpWriter)> {
        if let Some(metadata) = options.if_not_changed.take() {
            let (target, identity, name) = if capability.write_with_if_version_match
                && let Some(version) = metadata.version()
            {
                (&mut options.if_version_match, version, "if_version_match")
            } else if let Some(etag) = metadata.etag() {
                (&mut options.if_match, etag, "if_match")
            } else if let Some(version) = metadata.version() {
                (&mut options.if_version_match, version, "if_version_match")
            } else {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "if_not_changed metadata contains neither version nor ETag",
                )
                .with_operation(Operation::Write));
            };

            if let Some(explicit) = target {
                if explicit != identity {
                    return Err(Error::new(
                        ErrorKind::ConditionNotMatch,
                        format!("if_not_changed conflicts with {name}"),
                    )
                    .with_operation(Operation::Write));
                }
            } else {
                *target = Some(identity.to_owned());
            }
        }

        let writer = OpWriter {
            chunk: options.chunk,
        };
        let user_metadata = options.user_metadata.take().map(sorted_user_metadata);
        let mut flags = 0;
        if options.append {
            flags |= OP_WRITE_APPEND;
        }
        if options.if_not_exists {
            flags |= OP_WRITE_IF_NOT_EXISTS;
        }
        let fields = [
            options.content_type.as_deref().map(str::as_bytes),
            options.content_disposition.as_deref().map(str::as_bytes),
            options.content_encoding.as_deref().map(str::as_bytes),
            options.cache_control.as_deref().map(str::as_bytes),
            options.if_match.as_deref().map(str::as_bytes),
            options.if_none_match.as_deref().map(str::as_bytes),
            options.if_version_match.as_deref().map(str::as_bytes),
            options.if_version_not_match.as_deref().map(str::as_bytes),
            None,
        ];
        let mut lengths = fields.map(|value| value.map(<[u8]>::len));
        lengths[WriteField::UserMetadata as usize] =
            user_metadata.as_deref().map(user_metadata_encoded_len);
        let args = Self {
            concurrent: options.concurrent.max(1),
            flags,
            values: CompactValues::encode_with(&lengths, |field, output| match field {
                field if field == WriteField::UserMetadata as usize => {
                    write_user_metadata(
                        user_metadata.as_deref().expect("present field has a value"),
                        output,
                    );
                }
                _ => {
                    output.write(fields[field].expect("present field has a value"));
                }
            }),
        };
        Ok((args, writer))
    }

    /// Get the append from op.
    ///
    /// The append is the flag to indicate that this write operation is an append operation.
    #[inline]
    pub fn append(&self) -> bool {
        self.flags & OP_WRITE_APPEND != 0
    }

    /// Get the content type from option
    #[inline]
    pub fn content_type(&self) -> Option<&str> {
        string_value(&self.values, WriteField::ContentType as usize)
    }

    /// Get the content disposition from option
    #[inline]
    pub fn content_disposition(&self) -> Option<&str> {
        string_value(&self.values, WriteField::ContentDisposition as usize)
    }

    /// Get the content encoding from option
    #[inline]
    pub fn content_encoding(&self) -> Option<&str> {
        string_value(&self.values, WriteField::ContentEncoding as usize)
    }

    /// Get the cache control from option
    #[inline]
    pub fn cache_control(&self) -> Option<&str> {
        string_value(&self.values, WriteField::CacheControl as usize)
    }

    /// Get the concurrent.
    #[inline]
    pub fn concurrent(&self) -> usize {
        self.concurrent
    }

    /// Get If-Match from option
    #[inline]
    pub fn if_match(&self) -> Option<&str> {
        string_value(&self.values, WriteField::IfMatch as usize)
    }

    /// Get If-None-Match from option
    #[inline]
    pub fn if_none_match(&self) -> Option<&str> {
        string_value(&self.values, WriteField::IfNoneMatch as usize)
    }

    /// Get the version match condition.
    #[inline]
    pub fn if_version_match(&self) -> Option<&str> {
        string_value(&self.values, WriteField::IfVersionMatch as usize)
    }

    /// Get the version non-match condition.
    #[inline]
    pub fn if_version_not_match(&self) -> Option<&str> {
        string_value(&self.values, WriteField::IfVersionNotMatch as usize)
    }

    /// Get If-Not-Exist from option
    #[inline]
    pub fn if_not_exists(&self) -> bool {
        self.flags & OP_WRITE_IF_NOT_EXISTS != 0
    }

    /// Returns whether at least one conditional option is set.
    ///
    /// Value-bearing options are set when present. Boolean options are set only
    /// when their value is `true`.
    #[inline]
    pub fn is_conditional(&self) -> bool {
        self.if_not_exists()
            || self.if_match().is_some()
            || self.if_none_match().is_some()
            || self.if_version_match().is_some()
            || self.if_version_not_match().is_some()
    }

    /// Get the user defined metadata from the op
    #[inline]
    pub fn user_metadata(&self) -> Option<UserMetadata<'_>> {
        self.values
            .get(WriteField::UserMetadata as usize)
            .map(UserMetadata::new)
    }

    #[doc(hidden)]
    pub fn into_content_type(mut self, value: &str) -> Self {
        self.values = self
            .values
            .replace(WriteField::ContentType as usize, value.as_bytes());
        self
    }
}

/// Arguments for `writer` operation.
#[derive(Debug, Clone, Default)]
pub struct OpWriter {
    chunk: Option<usize>,
}

impl OpWriter {
    /// Create a new `OpWriter`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the chunk from op.
    ///
    /// The chunk is used by service to decide the chunk size of the underlying writer.
    pub fn chunk(&self) -> Option<usize> {
        self.chunk
    }

    /// Set the chunk of op.
    ///
    /// If chunk is set, the data will be chunked by the underlying writer.
    ///
    /// ## NOTE
    ///
    /// Service could have their own minimum chunk size while perform write
    /// operations like multipart uploads. So the chunk size may be larger than
    /// the given buffer size.
    pub fn with_chunk(mut self, chunk: usize) -> Self {
        self.chunk = Some(chunk);
        self
    }
}

/// Arguments for `compose` operation.
#[derive(Debug, Clone, Default)]
pub struct OpCompose {
    concurrent: usize,
    flags: u8,
    values: CompactValues,
}

const OP_COMPOSE_IF_NOT_EXISTS: u8 = 1;

#[repr(usize)]
enum ComposeField {
    ContentType,
    ContentDisposition,
    ContentEncoding,
    CacheControl,
    IfMatch,
    IfNoneMatch,
    IfVersionMatch,
    IfVersionNotMatch,
    UserMetadata,
}

impl OpCompose {
    /// Create a new `OpCompose`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lower `options` against `capability` and freeze them into service-ready arguments.
    ///
    /// # Errors
    ///
    /// Returns an error when `if_not_changed` contains no identity or conflicts
    /// with an explicit equality condition.
    ///
    /// # Panics
    ///
    /// Panics when the compact value block, including its index, exceeds `u16::MAX` bytes.
    pub fn from_options(
        capability: &Capability,
        mut options: options::ComposeOptions,
    ) -> Result<Self> {
        if let Some(metadata) = options.if_not_changed.take() {
            let (target, identity, name) = if capability.compose_with_if_version_match
                && let Some(version) = metadata.version()
            {
                (&mut options.if_version_match, version, "if_version_match")
            } else if let Some(etag) = metadata.etag() {
                (&mut options.if_match, etag, "if_match")
            } else if let Some(version) = metadata.version() {
                (&mut options.if_version_match, version, "if_version_match")
            } else {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "if_not_changed metadata contains neither version nor ETag",
                )
                .with_operation(Operation::Compose));
            };

            if let Some(explicit) = target {
                if explicit != identity {
                    return Err(Error::new(
                        ErrorKind::ConditionNotMatch,
                        format!("if_not_changed conflicts with {name}"),
                    )
                    .with_operation(Operation::Compose));
                }
            } else {
                *target = Some(identity.to_owned());
            }
        }

        let user_metadata = options.user_metadata.take().map(sorted_user_metadata);
        let fields = [
            options.content_type.as_deref().map(str::as_bytes),
            options.content_disposition.as_deref().map(str::as_bytes),
            options.content_encoding.as_deref().map(str::as_bytes),
            options.cache_control.as_deref().map(str::as_bytes),
            options.if_match.as_deref().map(str::as_bytes),
            options.if_none_match.as_deref().map(str::as_bytes),
            options.if_version_match.as_deref().map(str::as_bytes),
            options.if_version_not_match.as_deref().map(str::as_bytes),
            None,
        ];
        let mut lengths = fields.map(|value| value.map(<[u8]>::len));
        lengths[ComposeField::UserMetadata as usize] =
            user_metadata.as_deref().map(user_metadata_encoded_len);
        Ok(Self {
            concurrent: options.concurrent.max(1),
            flags: if options.if_not_exists {
                OP_COMPOSE_IF_NOT_EXISTS
            } else {
                0
            },
            values: CompactValues::encode_with(&lengths, |field, output| match field {
                field if field == ComposeField::UserMetadata as usize => {
                    write_user_metadata(
                        user_metadata.as_deref().expect("present field has a value"),
                        output,
                    );
                }
                _ => {
                    output.write(fields[field].expect("present field has a value"));
                }
            }),
        })
    }

    /// Get the maximum concurrent composition task count.
    pub fn concurrent(&self) -> usize {
        self.concurrent.max(1)
    }

    /// Get destination Content-Type metadata.
    pub fn content_type(&self) -> Option<&str> {
        string_value(&self.values, ComposeField::ContentType as usize)
    }

    /// Get destination Content-Disposition metadata.
    pub fn content_disposition(&self) -> Option<&str> {
        string_value(&self.values, ComposeField::ContentDisposition as usize)
    }

    /// Get destination Content-Encoding metadata.
    pub fn content_encoding(&self) -> Option<&str> {
        string_value(&self.values, ComposeField::ContentEncoding as usize)
    }

    /// Get destination Cache-Control metadata.
    pub fn cache_control(&self) -> Option<&str> {
        string_value(&self.values, ComposeField::CacheControl as usize)
    }

    /// Get destination user metadata.
    pub fn user_metadata(&self) -> Option<UserMetadata<'_>> {
        self.values
            .get(ComposeField::UserMetadata as usize)
            .map(UserMetadata::new)
    }

    /// Get the destination ETag match condition.
    pub fn if_match(&self) -> Option<&str> {
        string_value(&self.values, ComposeField::IfMatch as usize)
    }

    /// Get the destination ETag non-match condition.
    pub fn if_none_match(&self) -> Option<&str> {
        string_value(&self.values, ComposeField::IfNoneMatch as usize)
    }

    /// Get the destination version match condition.
    pub fn if_version_match(&self) -> Option<&str> {
        string_value(&self.values, ComposeField::IfVersionMatch as usize)
    }

    /// Get the destination version non-match condition.
    pub fn if_version_not_match(&self) -> Option<&str> {
        string_value(&self.values, ComposeField::IfVersionNotMatch as usize)
    }

    /// Get whether composition requires a missing destination.
    pub fn if_not_exists(&self) -> bool {
        self.flags & OP_COMPOSE_IF_NOT_EXISTS != 0
    }

    /// Returns whether at least one conditional option is set.
    ///
    /// Value-bearing options are set when present. Boolean options are set only
    /// when their value is `true`.
    #[inline]
    pub fn is_conditional(&self) -> bool {
        self.if_not_exists()
            || self.if_match().is_some()
            || self.if_none_match().is_some()
            || self.if_version_match().is_some()
            || self.if_version_not_match().is_some()
    }

    #[doc(hidden)]
    pub fn into_content_type(mut self, value: &str) -> Self {
        self.values = self
            .values
            .replace(ComposeField::ContentType as usize, value.as_bytes());
        self
    }
}

/// Arguments for `copy` operation.
#[derive(Debug, Clone, Default)]
pub struct OpCopy {
    concurrent: usize,
    chunk: Option<usize>,
    source_content_length_hint: Option<u64>,
    flags: u8,
    values: CompactValues,
}

const OP_COPY_IF_NOT_EXISTS: u8 = 1;

#[repr(usize)]
enum CopyField {
    IfMatch,
    IfNoneMatch,
    IfVersionMatch,
    IfVersionNotMatch,
    SourceVersion,
}

impl OpCopy {
    /// Create a new `OpCopy`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lower `options` against `capability` and freeze them into service-ready arguments.
    ///
    /// # Errors
    ///
    /// Returns an error when `if_not_changed` contains no identity or conflicts
    /// with an explicit equality condition.
    ///
    /// # Panics
    ///
    /// Panics when the compact value block, including its index, exceeds `u16::MAX` bytes.
    pub fn from_options(
        capability: &Capability,
        mut options: options::CopyOptions,
    ) -> Result<Self> {
        if let Some(metadata) = options.if_not_changed.take() {
            let (target, identity, name) = if capability.copy_with_if_version_match
                && let Some(version) = metadata.version()
            {
                (&mut options.if_version_match, version, "if_version_match")
            } else if let Some(etag) = metadata.etag() {
                (&mut options.if_match, etag, "if_match")
            } else if let Some(version) = metadata.version() {
                (&mut options.if_version_match, version, "if_version_match")
            } else {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "if_not_changed metadata contains neither version nor ETag",
                )
                .with_operation(Operation::Copy));
            };

            if let Some(explicit) = target {
                if explicit != identity {
                    return Err(Error::new(
                        ErrorKind::ConditionNotMatch,
                        format!("if_not_changed conflicts with {name}"),
                    )
                    .with_operation(Operation::Copy));
                }
            } else {
                *target = Some(identity.to_owned());
            }
        }

        let fields = [
            options.if_match.as_deref().map(str::as_bytes),
            options.if_none_match.as_deref().map(str::as_bytes),
            options.if_version_match.as_deref().map(str::as_bytes),
            options.if_version_not_match.as_deref().map(str::as_bytes),
            options.source_version.as_deref().map(str::as_bytes),
        ];
        Ok(Self {
            concurrent: options.concurrent.max(1),
            chunk: options.chunk,
            source_content_length_hint: options.source_content_length_hint,
            flags: if options.if_not_exists {
                OP_COPY_IF_NOT_EXISTS
            } else {
                0
            },
            values: CompactValues::encode(&fields),
        })
    }

    /// Get if_not_exists flag.
    #[inline]
    pub fn if_not_exists(&self) -> bool {
        self.flags & OP_COPY_IF_NOT_EXISTS != 0
    }

    /// Get if_match condition.
    #[inline]
    pub fn if_match(&self) -> Option<&str> {
        string_value(&self.values, CopyField::IfMatch as usize)
    }

    /// Get the destination ETag non-match condition.
    #[inline]
    pub fn if_none_match(&self) -> Option<&str> {
        string_value(&self.values, CopyField::IfNoneMatch as usize)
    }

    /// Get the current destination version match condition.
    #[inline]
    pub fn if_version_match(&self) -> Option<&str> {
        string_value(&self.values, CopyField::IfVersionMatch as usize)
    }

    /// Get the current destination version non-match condition.
    #[inline]
    pub fn if_version_not_match(&self) -> Option<&str> {
        string_value(&self.values, CopyField::IfVersionNotMatch as usize)
    }

    /// Returns whether at least one conditional option is set.
    ///
    /// Value-bearing options are set when present. Boolean options are set only
    /// when their value is `true`.
    #[inline]
    pub fn is_conditional(&self) -> bool {
        self.if_not_exists()
            || self.if_match().is_some()
            || self.if_none_match().is_some()
            || self.if_version_match().is_some()
            || self.if_version_not_match().is_some()
    }

    /// Get source version from the operation.
    #[inline]
    pub fn source_version(&self) -> Option<&str> {
        string_value(&self.values, CopyField::SourceVersion as usize)
    }

    /// Get the concurrent tasks for the copy operation.
    pub fn concurrent(&self) -> usize {
        self.concurrent.max(1)
    }

    /// Get the chunk size for the copy operation.
    pub fn chunk(&self) -> Option<usize> {
        self.chunk
    }

    /// Get source content length hint from the copy operation.
    pub fn source_content_length_hint(&self) -> Option<u64> {
        self.source_content_length_hint
    }
}

/// Arguments for `rename` operation.
#[derive(Debug, Clone, Default)]
pub struct OpRename {
    /// Whether the rename should fail when the destination already exists.
    ///
    /// If `true`, the rename succeeds only when the destination does not exist.
    /// If `false`, the rename uses OpenDAL's default overwrite behavior.
    if_not_exists: bool,
}

impl OpRename {
    /// Create a new `OpRename`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether the rename should fail when the destination already exists.
    ///
    /// If `true`, the rename succeeds only when the destination does not exist.
    /// If `false`, the rename uses OpenDAL's default overwrite behavior.
    ///
    /// ## Service Implementation
    ///
    /// Check [`crate::Capability::rename_with_if_not_exists`] before setting this to
    /// `true`. A service might return `ErrorKind::Unsupported` if it cannot
    /// enforce the condition.
    pub fn with_if_not_exists(mut self, if_not_exists: bool) -> Self {
        self.if_not_exists = if_not_exists;
        self
    }

    /// Return whether the rename should fail when the destination already exists.
    pub fn if_not_exists(&self) -> bool {
        self.if_not_exists
    }

    /// Returns whether the boolean conditional option is set to `true`.
    #[inline]
    pub fn is_conditional(&self) -> bool {
        self.if_not_exists()
    }
}

impl From<options::RenameOptions> for OpRename {
    fn from(value: options::RenameOptions) -> Self {
        Self {
            if_not_exists: value.if_not_exists,
        }
    }
}

/// Arguments for `restore` operation.
#[derive(Debug, Clone, Default)]
pub struct OpRestore {
    flags: u8,
    values: CompactValues,
}

const OP_RESTORE_IF_NOT_EXISTS: u8 = 1;

impl OpRestore {
    /// Create a new `OpRestore`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the version to restore.
    #[inline]
    pub fn version(&self) -> Option<&str> {
        string_value(&self.values, 0)
    }

    /// Return whether the restore should fail if the path currently exists.
    #[inline]
    pub fn if_not_exists(&self) -> bool {
        self.flags & OP_RESTORE_IF_NOT_EXISTS != 0
    }

    /// Returns whether the boolean conditional option is set to `true`.
    #[inline]
    pub fn is_conditional(&self) -> bool {
        self.if_not_exists()
    }
}

impl From<options::RestoreOptions> for OpRestore {
    fn from(value: options::RestoreOptions) -> Self {
        Self {
            flags: if value.if_not_exists {
                OP_RESTORE_IF_NOT_EXISTS
            } else {
                0
            },
            values: CompactValues::encode(&[value.version.as_deref().map(str::as_bytes)]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Metadata;
    use crate::MetadataBuilder;

    fn condition_metadata() -> Metadata {
        let mut metadata = MetadataBuilder::file(42);
        metadata
            .etag("etag")
            .version("version")
            .user_metadata([("owner".to_owned(), "opendal".to_owned())]);
        metadata.build()
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn compact_operation_layouts() {
        assert_eq!(size_of::<OpRead>(), 16);
        assert_eq!(size_of::<OpStat>(), 16);
        assert_eq!(size_of::<OpWrite>(), 32);
        assert_eq!(size_of::<OpCompose>(), 32);
        assert_eq!(size_of::<OpDelete>(), 24);
        assert_eq!(size_of::<OpCopy>(), 64);
        assert_eq!(size_of::<OpList>(), 32);
        assert_eq!(size_of::<OpRestore>(), 24);
    }

    #[test]
    fn read_options_roundtrip() {
        let modified = Timestamp::new(-1, -123).unwrap();
        let unmodified = Timestamp::new(2, 456).unwrap();
        let options = options::ReadOptions {
            version: Some("version".to_owned()),
            if_match: Some("etag".to_owned()),
            if_none_match: Some("other-etag".to_owned()),
            if_version_match: Some("version-match".to_owned()),
            if_version_not_match: Some("version-not-match".to_owned()),
            if_modified_since: Some(modified),
            if_unmodified_since: Some(unmodified),
            content_length_hint: Some(42),
            override_content_type: Some("text/plain".to_owned()),
            override_cache_control: Some("no-cache".to_owned()),
            override_content_disposition: Some("attachment".to_owned()),
            ..Default::default()
        };
        let (_, args, _) = options.into();

        assert_eq!(args.version(), Some("version"));
        assert_eq!(args.if_match(), Some("etag"));
        assert_eq!(args.if_none_match(), Some("other-etag"));
        assert_eq!(args.if_version_match(), Some("version-match"));
        assert_eq!(args.if_version_not_match(), Some("version-not-match"));
        assert_eq!(args.if_modified_since(), Some(modified));
        assert_eq!(args.if_unmodified_since(), Some(unmodified));
        assert_eq!(args.content_length_hint(), Some(42));
        assert_eq!(args.override_content_type(), Some("text/plain"));
        assert_eq!(args.override_cache_control(), Some("no-cache"));
        assert_eq!(args.override_content_disposition(), Some("attachment"));
        assert!(args.is_conditional());
    }

    #[test]
    fn write_options_preserve_owned_views() {
        let (args, _) = OpWrite::from_options(
            &Capability::default(),
            options::WriteOptions {
                append: true,
                concurrent: 4,
                content_type: Some("text/plain".to_owned()),
                content_disposition: Some("attachment".to_owned()),
                content_encoding: Some("gzip".to_owned()),
                cache_control: Some("no-cache".to_owned()),
                if_match: Some("etag".to_owned()),
                if_none_match: Some("other-etag".to_owned()),
                if_version_match: Some("version-match".to_owned()),
                if_version_not_match: Some("version-not-match".to_owned()),
                if_not_exists: true,
                user_metadata: Some(HashMap::from([("owner".to_owned(), "opendal".to_owned())])),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(args.append());
        assert_eq!(args.concurrent(), 4);
        assert_eq!(args.content_type(), Some("text/plain"));
        assert_eq!(args.content_disposition(), Some("attachment"));
        assert_eq!(args.content_encoding(), Some("gzip"));
        assert_eq!(args.cache_control(), Some("no-cache"));
        assert_eq!(args.if_match(), Some("etag"));
        assert_eq!(args.if_none_match(), Some("other-etag"));
        assert_eq!(args.if_version_match(), Some("version-match"));
        assert_eq!(args.if_version_not_match(), Some("version-not-match"));
        assert!(args.if_not_exists());
        assert_eq!(args.user_metadata().unwrap().get("owner"), Some("opendal"));
        assert!(args.is_conditional());
    }

    #[test]
    fn compose_options_preserve_owned_views() {
        let args = OpCompose::from_options(
            &Capability::default(),
            options::ComposeOptions {
                concurrent: 4,
                content_type: Some("text/plain".to_owned()),
                content_disposition: Some("attachment".to_owned()),
                content_encoding: Some("gzip".to_owned()),
                cache_control: Some("no-cache".to_owned()),
                if_match: Some("etag".to_owned()),
                if_none_match: Some("other-etag".to_owned()),
                if_version_match: Some("version-match".to_owned()),
                if_version_not_match: Some("version-not-match".to_owned()),
                if_not_exists: true,
                user_metadata: Some(HashMap::from([("owner".to_owned(), "opendal".to_owned())])),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(args.concurrent(), 4);
        assert_eq!(args.content_type(), Some("text/plain"));
        assert_eq!(args.content_disposition(), Some("attachment"));
        assert_eq!(args.content_encoding(), Some("gzip"));
        assert_eq!(args.cache_control(), Some("no-cache"));
        assert_eq!(args.if_match(), Some("etag"));
        assert_eq!(args.if_none_match(), Some("other-etag"));
        assert_eq!(args.if_version_match(), Some("version-match"));
        assert_eq!(args.if_version_not_match(), Some("version-not-match"));
        assert!(args.if_not_exists());
        assert_eq!(args.user_metadata().unwrap().get("owner"), Some("opendal"));
        assert!(args.is_conditional());
    }

    #[test]
    fn options_lower_if_not_changed_before_freeze() {
        let condition = condition_metadata();
        let capability = Capability {
            write_with_if_match: true,
            write_with_if_version_match: true,
            delete_with_if_match: true,
            copy_with_if_match: true,
            ..Default::default()
        };
        let (write, _) = OpWrite::from_options(
            &capability,
            options::WriteOptions {
                if_not_changed: Some(condition.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(write.if_match(), None);
        assert_eq!(write.if_version_match(), Some("version"));

        let delete = OpDelete::from_options(
            &capability,
            options::DeleteOptions {
                if_not_changed: Some(condition.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(delete.if_match(), Some("etag"));
        assert_eq!(delete.if_version_match(), None);

        let copy = OpCopy::from_options(
            &capability,
            options::CopyOptions {
                if_not_changed: Some(condition.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(copy.if_match(), Some("etag"));
        assert_eq!(copy.if_version_match(), None);

        let compose = OpCompose::from_options(
            &capability,
            options::ComposeOptions {
                if_not_changed: Some(condition.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(compose.if_match(), Some("etag"));
        assert_eq!(compose.if_version_match(), None);

        let compose_source = OpRead::from_compose_source_options(
            &capability,
            options::ComposeSourceOptions {
                if_not_changed: Some(condition.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(compose_source.if_match(), Some("etag"));
        assert_eq!(compose_source.version(), None);

        let compose_source = OpRead::from_compose_source_options(
            &Capability {
                compose_with_source_version: true,
                ..Default::default()
            },
            options::ComposeSourceOptions {
                if_not_changed: Some(condition),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(compose_source.if_match(), None);
        assert_eq!(compose_source.version(), Some("version"));
    }

    #[test]
    fn delete_copy_list_and_restore_options_roundtrip() {
        let delete = OpDelete::from_options(
            &Capability::default(),
            options::DeleteOptions {
                version: Some("version".to_owned()),
                recursive: true,
                if_match: Some("etag".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(delete.version(), Some("version"));
        assert!(delete.recursive());
        assert_eq!(delete.if_match(), Some("etag"));
        assert!(delete.is_conditional());

        let copy = OpCopy::from_options(
            &Capability::default(),
            options::CopyOptions {
                if_not_exists: true,
                if_version_match: Some("destination-version".to_owned()),
                source_version: Some("source-version".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(copy.if_not_exists());
        assert_eq!(copy.if_version_match(), Some("destination-version"));
        assert_eq!(copy.source_version(), Some("source-version"));
        assert!(copy.is_conditional());

        let list: OpList = options::ListOptions {
            limit: Some(100),
            start_after: Some("marker".to_owned()),
            recursive: true,
            versions: true,
            deleted: true,
        }
        .into();
        assert_eq!(list.limit(), Some(100));
        assert_eq!(list.start_after(), Some("marker"));
        assert!(list.recursive());
        assert!(list.versions());
        assert!(list.deleted());

        let restore: OpRestore = options::RestoreOptions {
            version: Some("version".to_owned()),
            if_not_exists: true,
        }
        .into();
        assert_eq!(restore.version(), Some("version"));
        assert!(restore.if_not_exists());
        assert!(restore.is_conditional());
    }
}
