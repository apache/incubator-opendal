/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

package org.apache.opendal;

import lombok.Data;

/**
 * Describes the operations and options supported by an operator's service stack.
 * A capability does not imply that the Java binding exposes the corresponding operation.
 * Read version and condition flags apply to both {@link ReadOptions} and {@link ReaderOptions}.
 */
@Data
public class Capability {

    /**
     * If operator supports stat.
     */
    public final boolean stat;

    /**
     * If operator supports stat with if matched.
     */
    public final boolean statWithIfMatch;

    /**
     * If operator supports stat with if none match.
     */
    public final boolean statWithIfNoneMatch;

    /**
     * If operator supports stat with if version match.
     */
    public final boolean statWithIfVersionMatch;

    /**
     * If operator supports stat with if version not match.
     */
    public final boolean statWithIfVersionNotMatch;

    /**
     * If operator supports stat with if modified since.
     */
    public final boolean statWithIfModifiedSince;

    /**
     * If operator supports stat with if unmodified since.
     */
    public final boolean statWithIfUnmodifiedSince;

    /**
     * Indicates if Cache-Control header override is supported during stat operations.
     */
    public final boolean statWithOverrideCacheControl;

    /**
     * Indicates if Content-Disposition header override is supported during stat operations.
     */
    public final boolean statWithOverrideContentDisposition;

    /**
     * Indicates if Content-Type header override is supported during stat operations.
     */
    public final boolean statWithOverrideContentType;

    /**
     * If operator supports stat with versions.
     */
    public final boolean statWithVersion;

    /**
     * If operator supports read.
     */
    public final boolean read;

    /**
     * If operator supports read with if matched.
     */
    public final boolean readWithIfMatch;

    /**
     * If operator supports read with if none match.
     */
    public final boolean readWithIfNoneMatch;

    /**
     * Indicates if conditional read operations using version match are supported.
     */
    public final boolean readWithIfVersionMatch;

    /**
     * Indicates if conditional read operations using version non-match are supported.
     */
    public final boolean readWithIfVersionNotMatch;

    /**
     * Indicates if conditional read operations using If-Modified-Since are supported.
     */
    public final boolean readWithIfModifiedSince;

    /**
     * Indicates if conditional read operations using If-Unmodified-Since are supported.
     */
    public final boolean readWithIfUnmodifiedSince;

    /**
     * If operator supports read with override cache control.
     */
    public final boolean readWithOverrideCacheControl;

    /**
     * if operator supports read with override content disposition.
     */
    public final boolean readWithOverrideContentDisposition;

    /**
     * if operator supports read with override content type.
     */
    public final boolean readWithOverrideContentType;

    /**
     * Indicates if versions read operations are supported.
     */
    public final boolean readWithVersion;

    /**
     * Indicates if suffix read operations are supported.
     */
    public final boolean readWithSuffix;

    /**
     * If operator supports write.
     */
    public final boolean write;

    /**
     * If operator supports write can be called in multi times.
     */
    public final boolean writeCanMulti;

    /**
     * Indicates if writing empty content is supported.
     */
    public final boolean writeCanEmpty;

    /**
     * If operator supports write by append.
     */
    public final boolean writeCanAppend;

    /**
     * Indicates if a non-append writer can natively copy source ranges into an in-progress write.
     * `Writer::copy_from` remains available through streaming fallback when false or when appending.
     */
    public final boolean writeCanCopyFrom;

    /**
     * If operator supports write with content type.
     */
    public final boolean writeWithContentType;

    /**
     * If operator supports write with content disposition.
     */
    public final boolean writeWithContentDisposition;

    /**
     * Indicates if Content-Encoding can be specified during write operations.
     */
    public final boolean writeWithContentEncoding;

    /**
     * If operator supports write with cache control.
     */
    public final boolean writeWithCacheControl;

    /**
     * If operator supports write with if match.
     */
    public final boolean writeWithIfMatch;

    /**
     * If operator supports write with if none match.
     *
     */
    public final boolean writeWithIfNoneMatch;

    /**
     * Indicates if conditional write operations using version match are supported.
     */
    public final boolean writeWithIfVersionMatch;

    /**
     * Indicates if conditional write operations using version non-match are supported.
     */
    public final boolean writeWithIfVersionNotMatch;

    /**
     * If operator supports write with if not exists.
     */
    public final boolean writeWithIfNotExists;

    /**
     * If operator supports write with user metadata.
     */
    public final boolean writeWithUserMetadata;

    /**
     * write_multi_max_size is the max size that services support in write_multi.
     * For example, AWS S3 supports 5GiB as max in write_multi.
     */
    public final long writeMultiMaxSize;

    /**
     * write_multi_min_size is the min size that services support in write_multi.
     * For example, AWS S3 requires at least 5MiB in write_multi expect the last one.
     */
    public final long writeMultiMinSize;

    /**
     * Maximum total size supported for write operations.
     * For example, Cloudflare D1 has a 1MB total size limit.
     * The value is -1 when the limit is unspecified.
     */
    public final long writeTotalMaxSize;

    /**
     * If operator supports create dir.
     */
    public final boolean createDir;

    /**
     * If operator supports delete.
     */
    public final boolean delete;

    /**
     * Indicates if versions delete operations are supported.
     */
    public final boolean deleteWithVersion;

    /**
     * Indicates if recursive delete operations are supported.
     */
    public final boolean deleteWithRecursive;

    /**
     * Indicates if conditional delete operations using If-Match are supported.
     */
    public final boolean deleteWithIfMatch;

    /**
     * Indicates if conditional delete operations using If-None-Match are supported.
     */
    public final boolean deleteWithIfNoneMatch;

    /**
     * Indicates if conditional delete operations using version match are supported.
     */
    public final boolean deleteWithIfVersionMatch;

    /**
     * Indicates if conditional delete operations using version non-match are supported.
     */
    public final boolean deleteWithIfVersionNotMatch;

    /**
     * Maximum size supported for single delete operations.
     * The value is -1 when the limit is unspecified.
     */
    public final long deleteMaxSize;

    /**
     * If operator supports copy.
     */
    public final boolean copy;

    /**
     * Indicates if conditional copy operations with if-not-exists are supported.
     */
    public final boolean copyWithIfNotExists;

    /**
     * Indicates if conditional copy operations with if-match are supported.
     */
    public final boolean copyWithIfMatch;

    /**
     * Indicates if conditional copy operations with if-none-match are supported.
     */
    public final boolean copyWithIfNoneMatch;

    /**
     * Indicates if conditional copy operations using version match are supported.
     */
    public final boolean copyWithIfVersionMatch;

    /**
     * Indicates if conditional copy operations using version non-match are supported.
     */
    public final boolean copyWithIfVersionNotMatch;

    /**
     * Indicates if copy operations from a specific source version are supported.
     */
    public final boolean copyWithSourceVersion;

    /**
     * Indicates if copy operations can be split into multiple server-side tasks.
     */
    public final boolean copyCanMulti;

    /**
     * Maximum size supported for segmented copy tasks.
     * The value is -1 when the limit is unspecified.
     */
    public final long copyMultiMaxSize;

    /**
     * Minimum size required for segmented copy tasks.
     * The value is -1 when the limit is unspecified.
     */
    public final long copyMultiMinSize;

    /**
     * Indicates if the operator supports composing complete source objects.
     */
    public final boolean compose;

    /**
     * Indicates if Content-Type can be specified during composition.
     */
    public final boolean composeWithContentType;

    /**
     * Indicates if Content-Disposition can be specified during composition.
     */
    public final boolean composeWithContentDisposition;

    /**
     * Indicates if Content-Encoding can be specified during composition.
     */
    public final boolean composeWithContentEncoding;

    /**
     * Indicates if Cache-Control can be specified during composition.
     */
    public final boolean composeWithCacheControl;

    /**
     * Indicates if custom user metadata can be attached during composition.
     */
    public final boolean composeWithUserMetadata;

    /**
     * Indicates if composition supports a destination ETag match condition.
     */
    public final boolean composeWithIfMatch;

    /**
     * Indicates if composition supports a destination ETag non-match condition.
     */
    public final boolean composeWithIfNoneMatch;

    /**
     * Indicates if composition supports a destination version match condition.
     */
    public final boolean composeWithIfVersionMatch;

    /**
     * Indicates if composition supports a destination version non-match condition.
     */
    public final boolean composeWithIfVersionNotMatch;

    /**
     * Indicates if composition can require a missing destination.
     */
    public final boolean composeWithIfNotExists;

    /**
     * Indicates if composition can select a specific source version.
     */
    public final boolean composeWithSourceVersion;

    /**
     * Indicates if composition supports a source ETag match condition.
     */
    public final boolean composeWithSourceIfMatch;

    /**
     * Indicates if restore operations are supported.
     */
    public final boolean restore;

    /**
     * Indicates if restoring a specific version is supported.
     */
    public final boolean restoreWithVersion;

    /**
     * Indicates if conditional restore operations using if-not-exists are supported.
     */
    public final boolean restoreWithIfNotExists;

    /**
     * If operator supports rename.
     */
    public final boolean rename;

    /**
     * Indicates if conditional rename operations with if-not-exists are supported.
     */
    public final boolean renameWithIfNotExists;

    /**
     * If operator supports list.
     */
    public final boolean list;

    /**
     * If backend supports list with limit.
     */
    public final boolean listWithLimit;

    /**
     * If backend supports list with start after.
     */
    public final boolean listWithStartAfter;

    /**
     * If backend support list with recursive.
     */
    public final boolean listWithRecursive;

    /**
     * If backend support list with versions.
     */
    public final boolean listWithVersions;

    /**
     * If backend support list with deleted.
     */
    public final boolean listWithDeleted;

    /**
     * If operator supports presign.
     */
    public final boolean presign;

    /**
     * If operator supports presign read.
     */
    public final boolean presignRead;

    /**
     * If operator supports presign stat.
     */
    public final boolean presignStat;

    /**
     * If operator supports presign write.
     */
    public final boolean presignWrite;

    /**
     * Indicates if presigned URLs for delete operations are supported.
     */
    public final boolean presignDelete;

    /**
     * If operator supports shared.
     */
    public final boolean shared;

    /**
     * Constructs a capability using the original field set.
     * Additional capability flags default to false and additional limits to -1.
     */
    public Capability(
            boolean stat,
            boolean statWithIfMatch,
            boolean statWithIfNoneMatch,
            boolean statWithIfVersionMatch,
            boolean statWithIfVersionNotMatch,
            boolean statWithIfModifiedSince,
            boolean statWithIfUnmodifiedSince,
            boolean statWithVersion,
            boolean read,
            boolean readWithIfMatch,
            boolean readWithIfNoneMatch,
            boolean readWithOverrideCacheControl,
            boolean readWithOverrideContentDisposition,
            boolean readWithOverrideContentType,
            boolean write,
            boolean writeCanMulti,
            boolean writeCanAppend,
            boolean writeWithContentType,
            boolean writeWithContentDisposition,
            boolean writeWithCacheControl,
            boolean writeWithIfMatch,
            boolean writeWithIfNoneMatch,
            boolean writeWithIfNotExists,
            boolean writeWithUserMetadata,
            long writeMultiMaxSize,
            long writeMultiMinSize,
            boolean createDir,
            boolean delete,
            boolean copy,
            boolean rename,
            boolean list,
            boolean listWithLimit,
            boolean listWithStartAfter,
            boolean listWithRecursive,
            boolean listWithVersions,
            boolean listWithDeleted,
            boolean presign,
            boolean presignRead,
            boolean presignStat,
            boolean presignWrite,
            boolean shared) {
        this(
                stat,
                statWithIfMatch,
                statWithIfNoneMatch,
                statWithIfVersionMatch,
                statWithIfVersionNotMatch,
                statWithIfModifiedSince,
                statWithIfUnmodifiedSince,
                false,
                false,
                false,
                statWithVersion,
                read,
                readWithIfMatch,
                readWithIfNoneMatch,
                false,
                false,
                false,
                false,
                readWithOverrideCacheControl,
                readWithOverrideContentDisposition,
                readWithOverrideContentType,
                false,
                false,
                write,
                writeCanMulti,
                false,
                writeCanAppend,
                false,
                writeWithContentType,
                writeWithContentDisposition,
                false,
                writeWithCacheControl,
                writeWithIfMatch,
                writeWithIfNoneMatch,
                false,
                false,
                writeWithIfNotExists,
                writeWithUserMetadata,
                writeMultiMaxSize,
                writeMultiMinSize,
                -1L,
                createDir,
                delete,
                false,
                false,
                false,
                false,
                false,
                false,
                -1L,
                copy,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                -1L,
                -1L,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                rename,
                false,
                list,
                listWithLimit,
                listWithStartAfter,
                listWithRecursive,
                listWithVersions,
                listWithDeleted,
                presign,
                presignRead,
                presignStat,
                presignWrite,
                false,
                shared);
    }

    /** Constructs a capability using all fields exposed by the Rust core. */
    public Capability(
            boolean stat,
            boolean statWithIfMatch,
            boolean statWithIfNoneMatch,
            boolean statWithIfVersionMatch,
            boolean statWithIfVersionNotMatch,
            boolean statWithIfModifiedSince,
            boolean statWithIfUnmodifiedSince,
            boolean statWithOverrideCacheControl,
            boolean statWithOverrideContentDisposition,
            boolean statWithOverrideContentType,
            boolean statWithVersion,
            boolean read,
            boolean readWithIfMatch,
            boolean readWithIfNoneMatch,
            boolean readWithIfVersionMatch,
            boolean readWithIfVersionNotMatch,
            boolean readWithIfModifiedSince,
            boolean readWithIfUnmodifiedSince,
            boolean readWithOverrideCacheControl,
            boolean readWithOverrideContentDisposition,
            boolean readWithOverrideContentType,
            boolean readWithVersion,
            boolean readWithSuffix,
            boolean write,
            boolean writeCanMulti,
            boolean writeCanEmpty,
            boolean writeCanAppend,
            boolean writeCanCopyFrom,
            boolean writeWithContentType,
            boolean writeWithContentDisposition,
            boolean writeWithContentEncoding,
            boolean writeWithCacheControl,
            boolean writeWithIfMatch,
            boolean writeWithIfNoneMatch,
            boolean writeWithIfVersionMatch,
            boolean writeWithIfVersionNotMatch,
            boolean writeWithIfNotExists,
            boolean writeWithUserMetadata,
            long writeMultiMaxSize,
            long writeMultiMinSize,
            long writeTotalMaxSize,
            boolean createDir,
            boolean delete,
            boolean deleteWithVersion,
            boolean deleteWithRecursive,
            boolean deleteWithIfMatch,
            boolean deleteWithIfNoneMatch,
            boolean deleteWithIfVersionMatch,
            boolean deleteWithIfVersionNotMatch,
            long deleteMaxSize,
            boolean copy,
            boolean copyWithIfNotExists,
            boolean copyWithIfMatch,
            boolean copyWithIfNoneMatch,
            boolean copyWithIfVersionMatch,
            boolean copyWithIfVersionNotMatch,
            boolean copyWithSourceVersion,
            boolean copyCanMulti,
            long copyMultiMaxSize,
            long copyMultiMinSize,
            boolean compose,
            boolean composeWithContentType,
            boolean composeWithContentDisposition,
            boolean composeWithContentEncoding,
            boolean composeWithCacheControl,
            boolean composeWithUserMetadata,
            boolean composeWithIfMatch,
            boolean composeWithIfNoneMatch,
            boolean composeWithIfVersionMatch,
            boolean composeWithIfVersionNotMatch,
            boolean composeWithIfNotExists,
            boolean composeWithSourceVersion,
            boolean composeWithSourceIfMatch,
            boolean restore,
            boolean restoreWithVersion,
            boolean restoreWithIfNotExists,
            boolean rename,
            boolean renameWithIfNotExists,
            boolean list,
            boolean listWithLimit,
            boolean listWithStartAfter,
            boolean listWithRecursive,
            boolean listWithVersions,
            boolean listWithDeleted,
            boolean presign,
            boolean presignRead,
            boolean presignStat,
            boolean presignWrite,
            boolean presignDelete,
            boolean shared) {
        this.stat = stat;
        this.statWithIfMatch = statWithIfMatch;
        this.statWithIfNoneMatch = statWithIfNoneMatch;
        this.statWithIfVersionMatch = statWithIfVersionMatch;
        this.statWithIfVersionNotMatch = statWithIfVersionNotMatch;
        this.statWithIfModifiedSince = statWithIfModifiedSince;
        this.statWithIfUnmodifiedSince = statWithIfUnmodifiedSince;
        this.statWithOverrideCacheControl = statWithOverrideCacheControl;
        this.statWithOverrideContentDisposition = statWithOverrideContentDisposition;
        this.statWithOverrideContentType = statWithOverrideContentType;
        this.statWithVersion = statWithVersion;
        this.read = read;
        this.readWithIfMatch = readWithIfMatch;
        this.readWithIfNoneMatch = readWithIfNoneMatch;
        this.readWithIfVersionMatch = readWithIfVersionMatch;
        this.readWithIfVersionNotMatch = readWithIfVersionNotMatch;
        this.readWithIfModifiedSince = readWithIfModifiedSince;
        this.readWithIfUnmodifiedSince = readWithIfUnmodifiedSince;
        this.readWithOverrideCacheControl = readWithOverrideCacheControl;
        this.readWithOverrideContentDisposition = readWithOverrideContentDisposition;
        this.readWithOverrideContentType = readWithOverrideContentType;
        this.readWithVersion = readWithVersion;
        this.readWithSuffix = readWithSuffix;
        this.write = write;
        this.writeCanMulti = writeCanMulti;
        this.writeCanEmpty = writeCanEmpty;
        this.writeCanAppend = writeCanAppend;
        this.writeCanCopyFrom = writeCanCopyFrom;
        this.writeWithContentType = writeWithContentType;
        this.writeWithContentDisposition = writeWithContentDisposition;
        this.writeWithContentEncoding = writeWithContentEncoding;
        this.writeWithCacheControl = writeWithCacheControl;
        this.writeWithIfMatch = writeWithIfMatch;
        this.writeWithIfNoneMatch = writeWithIfNoneMatch;
        this.writeWithIfVersionMatch = writeWithIfVersionMatch;
        this.writeWithIfVersionNotMatch = writeWithIfVersionNotMatch;
        this.writeWithIfNotExists = writeWithIfNotExists;
        this.writeWithUserMetadata = writeWithUserMetadata;
        this.writeMultiMaxSize = writeMultiMaxSize;
        this.writeMultiMinSize = writeMultiMinSize;
        this.writeTotalMaxSize = writeTotalMaxSize;
        this.createDir = createDir;
        this.delete = delete;
        this.deleteWithVersion = deleteWithVersion;
        this.deleteWithRecursive = deleteWithRecursive;
        this.deleteWithIfMatch = deleteWithIfMatch;
        this.deleteWithIfNoneMatch = deleteWithIfNoneMatch;
        this.deleteWithIfVersionMatch = deleteWithIfVersionMatch;
        this.deleteWithIfVersionNotMatch = deleteWithIfVersionNotMatch;
        this.deleteMaxSize = deleteMaxSize;
        this.copy = copy;
        this.copyWithIfNotExists = copyWithIfNotExists;
        this.copyWithIfMatch = copyWithIfMatch;
        this.copyWithIfNoneMatch = copyWithIfNoneMatch;
        this.copyWithIfVersionMatch = copyWithIfVersionMatch;
        this.copyWithIfVersionNotMatch = copyWithIfVersionNotMatch;
        this.copyWithSourceVersion = copyWithSourceVersion;
        this.copyCanMulti = copyCanMulti;
        this.copyMultiMaxSize = copyMultiMaxSize;
        this.copyMultiMinSize = copyMultiMinSize;
        this.compose = compose;
        this.composeWithContentType = composeWithContentType;
        this.composeWithContentDisposition = composeWithContentDisposition;
        this.composeWithContentEncoding = composeWithContentEncoding;
        this.composeWithCacheControl = composeWithCacheControl;
        this.composeWithUserMetadata = composeWithUserMetadata;
        this.composeWithIfMatch = composeWithIfMatch;
        this.composeWithIfNoneMatch = composeWithIfNoneMatch;
        this.composeWithIfVersionMatch = composeWithIfVersionMatch;
        this.composeWithIfVersionNotMatch = composeWithIfVersionNotMatch;
        this.composeWithIfNotExists = composeWithIfNotExists;
        this.composeWithSourceVersion = composeWithSourceVersion;
        this.composeWithSourceIfMatch = composeWithSourceIfMatch;
        this.restore = restore;
        this.restoreWithVersion = restoreWithVersion;
        this.restoreWithIfNotExists = restoreWithIfNotExists;
        this.rename = rename;
        this.renameWithIfNotExists = renameWithIfNotExists;
        this.list = list;
        this.listWithLimit = listWithLimit;
        this.listWithStartAfter = listWithStartAfter;
        this.listWithRecursive = listWithRecursive;
        this.listWithVersions = listWithVersions;
        this.listWithDeleted = listWithDeleted;
        this.presign = presign;
        this.presignRead = presignRead;
        this.presignStat = presignStat;
        this.presignWrite = presignWrite;
        this.presignDelete = presignDelete;
        this.shared = shared;
    }
}
