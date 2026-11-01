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

use napi::bindgen_prelude::BigInt;
use opendal::BytesRange;
use opendal::raw::Timestamp;
use std::collections::HashMap;

#[napi(object)]
#[derive(Debug)]
pub struct StatOptions {
    /**
     * Sets version for this operation.
     * Retrieves data of a specified version of the given path.
     */
    pub version: Option<String>,

    /**
     * Sets if-match condition for this operation.
     * If file exists and its etag doesn't match, an error will be returned.
     */
    pub if_match: Option<String>,

    /**
     * Sets if-none-match condition for this operation.
     * If file exists and its etag matches, an error will be returned.
     */
    pub if_none_match: Option<String>,

    /**
     * Sets if-modified-since condition for this operation.
     * If file exists and hasn't been modified since the specified time, an error will be returned.
     * ISO 8601 formatted date string
     * https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toISOString
     */
    pub if_modified_since: Option<String>,

    /**
     * Sets if-unmodified-since condition for this operation.
     * If file exists and has been modified since the specified time, an error will be returned.
     * ISO 8601 formatted date string
     * https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toISOString
     */
    pub if_unmodified_since: Option<String>,

    /**
     * Specifies the content-type header for presigned operations.
     * Only meaningful when used along with presign.
     */
    pub override_content_type: Option<String>,

    /**
     * Specifies the cache-control header for presigned operations.
     * Only meaningful when used along with presign.
     */
    pub override_cache_control: Option<String>,

    /**
     * Specifies the content-disposition header for presigned operations.
     * Only meaningful when used along with presign.
     */
    pub override_content_disposition: Option<String>,
}

impl From<StatOptions> for opendal::options::StatOptions {
    fn from(value: StatOptions) -> Self {
        let if_modified_since = value
            .if_modified_since
            .and_then(|v| v.parse::<Timestamp>().ok());
        let if_unmodified_since = value
            .if_unmodified_since
            .and_then(|v| v.parse::<Timestamp>().ok());

        Self {
            if_modified_since,
            if_unmodified_since,
            version: value.version,
            if_match: value.if_match,
            if_none_match: value.if_none_match,
            override_content_type: value.override_content_type,
            override_cache_control: value.override_cache_control,
            override_content_disposition: value.override_content_disposition,
            ..Default::default()
        }
    }
}

#[napi(object)]
#[derive(Default, Debug)]
pub struct ReadOptions {
    /**
     * Set `version` for this operation.
     *
     * This option can be used to retrieve the data of a specified version of the given path.
     */
    pub version: Option<String>,

    /**
     * Set `concurrent` for the operation.
     *
     * OpenDAL by default to read file without concurrent. This is not efficient for cases when users
     * read large chunks of data. By setting `concurrent`, opendal will reading files concurrently
     * on support storage services.
     *
     * By setting `concurrent`, opendal will fetch chunks concurrently with
     * the give chunk size.
     */
    pub concurrent: Option<u32>,

    /**
     * Sets the chunk size for this operation.
     *
     * OpenDAL will use services' preferred chunk size by default. Users can set chunk based on their own needs.
     */
    pub chunk: Option<u32>,

    /**
     * Controls the optimization strategy for range reads in [`Reader::fetch`].
     *
     * When performing range reads, if the gap between two requested ranges is smaller than
     * the configured `gap` size, OpenDAL will merge these ranges into a single read request
     * and discard the unrequested data in between. This helps reduce the number of API calls
     * to remote storage services.
     *
     * This optimization is particularly useful when performing multiple small range reads
     * that are close to each other, as it reduces the overhead of multiple network requests
     * at the cost of transferring some additional data.
     */
    pub gap: Option<BigInt>,

    /**
     * Sets the offset (starting position) for range read operations.
     * The read will start from this position in the file.
     */
    pub offset: Option<BigInt>,

    /**
     * Sets the size (length) for range read operations.
     * The read will continue for this many bytes after the offset.
     */
    pub size: Option<BigInt>,

    /**
     * Sets if-match condition for this operation.
     * If file exists and its etag doesn't match, an error will be returned.
     */
    pub if_match: Option<String>,

    /**
     * Sets if-none-match condition for this operation.
     * If file exists and its etag matches, an error will be returned.
     */
    pub if_none_match: Option<String>,

    /**
     * Sets if-modified-since condition for this operation.
     * If file exists and hasn't been modified since the specified time, an error will be returned.
     * ISO 8601 formatted date string
     * https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toISOString
     */
    pub if_modified_since: Option<String>,

    /**
     * Sets if-unmodified-since condition for this operation.
     * If file exists and has been modified since the specified time, an error will be returned.
     * ISO 8601 formatted date string
     * https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toISOString
     */
    pub if_unmodified_since: Option<String>,

    /**
     * Specify the `content-type` header that should be sent back by the operation.
     *
     * This option is only meaningful when used along with presign.
     */
    pub content_type: Option<String>,

    /**
     * Specify the `cache-control` header that should be sent back by the operation.
     *
     * This option is only meaningful when used along with presign.
     */
    pub cache_control: Option<String>,

    /**
     * Specify the `content-disposition` header that should be sent back by the operation.
     *
     * This option is only meaningful when used along with presign.
     */
    pub content_disposition: Option<String>,
}

impl ReadOptions {
    pub fn make_range(&self) -> BytesRange {
        let offset = self.offset.clone().map(|offset| offset.get_u64().1);
        let size = self.size.clone().map(|size| size.get_u64().1);
        BytesRange::new(offset.unwrap_or_default(), size)
    }
}

impl From<ReadOptions> for opendal::options::ReadOptions {
    fn from(value: ReadOptions) -> Self {
        let range = value.make_range();
        let if_modified_since = value
            .if_modified_since
            .and_then(|v| v.parse::<Timestamp>().ok());
        let if_unmodified_since = value
            .if_unmodified_since
            .and_then(|v| v.parse::<Timestamp>().ok());

        Self {
            version: value.version,
            concurrent: value.concurrent.unwrap_or_default() as usize,
            chunk: value.chunk.map(|chunk| chunk as usize),
            gap: value.gap.map(|gap| gap.get_u64().1 as usize),
            range,
            if_match: value.if_match,
            if_none_match: value.if_none_match,
            if_modified_since,
            if_unmodified_since,
            content_length_hint: None,
            override_content_type: value.content_type,
            override_cache_control: value.cache_control,
            override_content_disposition: value.content_disposition,
            ..Default::default()
        }
    }
}

#[napi(object)]
#[derive(Default)]
pub struct ReaderOptions {
    /**
     * Set `version` for this operation.
     *
     * This option can be used to retrieve the data of a specified version of the given path.
     */
    pub version: Option<String>,

    /**
     * Set `concurrent` for the operation.
     *
     * OpenDAL by default to read file without concurrent. This is not efficient for cases when users
     * read large chunks of data. By setting `concurrent`, opendal will reading files concurrently
     * on support storage services.
     *
     * By setting `concurrent`, opendal will fetch chunks concurrently with
     * the give chunk size.
     */
    pub concurrent: Option<u32>,

    /**
     * Sets the chunk size for this operation.
     *
     * OpenDAL will use services' preferred chunk size by default. Users can set chunk based on their own needs.
     */
    pub chunk: Option<u32>,

    /** Controls the number of prefetched bytes ranges that can be buffered in memory
     * during concurrent reading.
     *
     * When performing concurrent reads with `Reader`, this option limits how many
     * completed-but-not-yet-read chunks can be buffered. Once the number of buffered
     * chunks reaches this limit, no new read tasks will be spawned until some of the
     * buffered chunks are consumed.
     *
     * - Default value: 0 (no prefetching, strict back-pressure control)
     * - Set to a higher value to allow more aggressive prefetching at the cost of memory
     *
     * This option helps prevent memory exhaustion when reading large files with high
     * concurrency settings.
     */
    pub prefetch: Option<u32>,

    /**
     * Controls the optimization strategy for range reads in [`Reader::fetch`].
     *
     * When performing range reads, if the gap between two requested ranges is smaller than
     * the configured `gap` size, OpenDAL will merge these ranges into a single read request
     * and discard the unrequested data in between. This helps reduce the number of API calls
     * to remote storage services.
     *
     * This optimization is particularly useful when performing multiple small range reads
     * that are close to each other, as it reduces the overhead of multiple network requests
     * at the cost of transferring some additional data.
     */
    pub gap: Option<BigInt>,

    /**
     * Sets if-match condition for this operation.
     * If file exists and its etag doesn't match, an error will be returned.
     */
    pub if_match: Option<String>,

    /**
     * Sets if-none-match condition for this operation.
     * If file exists and its etag matches, an error will be returned.
     */
    pub if_none_match: Option<String>,

    /**
     * Sets if-modified-since condition for this operation.
     * If file exists and hasn't been modified since the specified time, an error will be returned.
     * ISO 8601 formatted date string
     * https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toISOString
     */
    pub if_modified_since: Option<String>,

    /**
     * Sets if-unmodified-since condition for this operation.
     * If file exists and has been modified since the specified time, an error will be returned.
     * ISO 8601 formatted date string
     * https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/toISOString
     */
    pub if_unmodified_since: Option<String>,
}

impl From<ReaderOptions> for opendal::options::ReaderOptions {
    fn from(value: ReaderOptions) -> Self {
        let if_modified_since = value
            .if_modified_since
            .and_then(|v| v.parse::<Timestamp>().ok());
        let if_unmodified_since = value
            .if_unmodified_since
            .and_then(|v| v.parse::<Timestamp>().ok());

        Self {
            version: value.version,
            concurrent: value.concurrent.unwrap_or_default() as usize,
            chunk: value.chunk.map(|chunk| chunk as usize),
            gap: value.gap.map(|gap| gap.get_u64().1 as usize),
            prefetch: value.prefetch.unwrap_or_default() as usize,
            if_match: value.if_match,
            if_none_match: value.if_none_match,
            if_modified_since,
            if_unmodified_since,
            content_length_hint: None,
            ..Default::default()
        }
    }
}

#[napi(object)]
#[derive(Debug, Default)]
pub struct ListOptions {
    /**
     * The limit passed to underlying service to specify the max results
     * that could return per-request.
     *
     * Users could use this to control the memory usage of list operation.
     */
    pub limit: Option<u32>,

    /**
     * The start_after passed to underlying service to specify the specified key
     * to start listing from.
     */
    pub start_after: Option<String>,

    /**
     * The recursive is used to control whether the list operation is recursive.
     *
     * - If `false`, list operation will only list the entries under the given path.
     * - If `true`, list operation will list all entries that starts with given path.
     *
     * Default to `false`.
     */
    pub recursive: Option<bool>,

    /**
     * The versions is used to control whether the object versions should be returned.
     *
     * - If `false`, list operation will not return with object versions
     * - If `true`, list operation will return with object versions if object versioning is supported
     *   by the underlying service
     *
     * Default to `false`
     */
    pub versions: Option<bool>,

    /**
     * The deleted is used to control whether the deleted objects should be returned.
     *
     * - If `false`, list operation will not return with deleted objects
     * - If `true`, list operation will return with deleted objects if object versioning is supported
     *   by the underlying service
     *
     * Default to `false`
     */
    pub deleted: Option<bool>,
}

impl From<ListOptions> for opendal::options::ListOptions {
    fn from(value: ListOptions) -> Self {
        Self {
            limit: value.limit.map(|v| v as usize),
            start_after: value.start_after,
            recursive: value.recursive.unwrap_or_default(),
            versions: value.versions.unwrap_or_default(),
            deleted: value.deleted.unwrap_or_default(),
        }
    }
}

#[napi(object)]
#[derive(Default, Debug)]
pub struct WriteOptions {
    /// Append bytes into a path.
    ///
    /// ### Notes
    ///
    /// - It always appends content to the end of the file.
    /// - It will create file if the path does not exist.
    pub append: Option<bool>,

    /// Set the chunk of op.
    ///
    /// If chunk is set, the data will be chunked by the underlying writer.
    ///
    /// ## NOTE
    ///
    /// A service could have their own minimum chunk size while perform write
    /// operations like multipart uploads. So the chunk size may be larger than
    /// the given buffer size.
    pub chunk: Option<BigInt>,

    /// Set the [Content-Type](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type) of op.
    pub content_type: Option<String>,

    /// Set the [Content-Disposition](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Disposition) of op.
    pub content_disposition: Option<String>,

    /// Set the [Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control) of op.
    pub cache_control: Option<String>,

    /// Set the [Content-Encoding] https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Encoding of op.
    pub content_encoding: Option<String>,

    /// Sets user metadata of op.
    ///
    /// If chunk is set, the user metadata will be attached to the object during write.
    ///
    /// ## NOTE
    ///
    /// - Services may have limitations for user metadata, for example:
    ///   - Key length is typically limited (e.g., 1024 bytes)
    ///   - Value length is typically limited (e.g., 4096 bytes)
    ///   - Total metadata size might be limited
    ///   - Some characters might be forbidden in keys
    pub user_metadata: Option<HashMap<String, String>>,

    /// Sets if-match condition of op.
    ///
    /// This operation provides conditional write functionality based on ETag matching,
    /// helping prevent unintended overwrites in concurrent scenarios.
    pub if_match: Option<String>,

    /// Sets if-none-match condition of op.
    ///
    /// This operation provides conditional write functionality based on ETag non-matching,
    /// useful for preventing overwriting existing resources or ensuring unique writes.
    pub if_none_match: Option<String>,

    /// Sets if_not_exists condition of op.
    ///
    /// This operation provides a way to ensure write operations only create new resources
    /// without overwriting existing ones, useful for implementing "create if not exists" logic.
    pub if_not_exists: Option<bool>,

    /// Sets concurrent of op.
    ///
    /// - By default, OpenDAL writes files sequentially
    /// - When concurrent is set:
    ///   - Multiple write operations can execute in parallel
    ///   - Write operations return immediately without waiting if tasks space are available
    ///   - Close operation ensures all writes complete in order
    ///   - Memory usage increases with concurrency level
    pub concurrent: Option<u32>,
}

impl From<WriteOptions> for opendal::options::WriteOptions {
    fn from(value: WriteOptions) -> Self {
        Self {
            append: value.append.unwrap_or_default(),
            chunk: value.chunk.map(|v| v.get_u64().1 as usize),
            content_type: value.content_type,
            content_disposition: value.content_disposition,
            cache_control: value.cache_control,
            content_encoding: value.content_encoding,
            user_metadata: value.user_metadata,
            if_match: value.if_match,
            if_none_match: value.if_none_match,
            if_not_exists: value.if_not_exists.unwrap_or_default(),
            concurrent: value.concurrent.unwrap_or_default() as usize,
            ..Default::default()
        }
    }
}

#[napi(object)]
#[derive(Default)]
pub struct DeleteOptions {
    pub version: Option<String>,
    /// Whether to delete recursively.
    pub recursive: Option<bool>,
    /**
     * Sets if-match condition for this operation.
     * If file exists and its etag does not match, an error of kind
     * `ConditionNotMatch` will be returned.
     */
    pub if_match: Option<String>,
}

impl From<DeleteOptions> for opendal::options::DeleteOptions {
    fn from(value: DeleteOptions) -> Self {
        Self {
            version: value.version,
            recursive: value.recursive.unwrap_or_default(),
            if_match: value.if_match,
            ..Default::default()
        }
    }
}
