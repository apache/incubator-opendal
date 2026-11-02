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
     * If operator supports write.
     */
    public final boolean write;

    /**
     * If operator supports write can be called in multi times.
     */
    public final boolean writeCanMulti;

    /**
     * If operator supports write by append.
     */
    public final boolean writeCanAppend;

    /**
     * If operator supports write with content type.
     */
    public final boolean writeWithContentType;

    /**
     * If operator supports write with content disposition.
     */
    public final boolean writeWithContentDisposition;

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
     * If operator supports create dir.
     */
    public final boolean createDir;

    /**
     * If operator supports delete.
     */
    public final boolean delete;

    /**
     * If operator supports copy.
     */
    public final boolean copy;

    /**
     * If operator supports rename.
     */
    public final boolean rename;

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
     * If operator supports shared.
     */
    public final boolean shared;

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
        this.stat = stat;
        this.statWithIfMatch = statWithIfMatch;
        this.statWithIfNoneMatch = statWithIfNoneMatch;
        this.statWithIfVersionMatch = statWithIfVersionMatch;
        this.statWithIfVersionNotMatch = statWithIfVersionNotMatch;
        this.statWithIfModifiedSince = statWithIfModifiedSince;
        this.statWithIfUnmodifiedSince = statWithIfUnmodifiedSince;
        this.statWithVersion = statWithVersion;
        this.read = read;
        this.readWithIfMatch = readWithIfMatch;
        this.readWithIfNoneMatch = readWithIfNoneMatch;
        this.readWithOverrideCacheControl = readWithOverrideCacheControl;
        this.readWithOverrideContentDisposition = readWithOverrideContentDisposition;
        this.readWithOverrideContentType = readWithOverrideContentType;
        this.write = write;
        this.writeCanMulti = writeCanMulti;
        this.writeCanAppend = writeCanAppend;
        this.writeWithContentType = writeWithContentType;
        this.writeWithContentDisposition = writeWithContentDisposition;
        this.writeWithCacheControl = writeWithCacheControl;
        this.writeWithIfMatch = writeWithIfMatch;
        this.writeWithIfNoneMatch = writeWithIfNoneMatch;
        this.writeWithIfNotExists = writeWithIfNotExists;
        this.writeWithUserMetadata = writeWithUserMetadata;
        this.writeMultiMaxSize = writeMultiMaxSize;
        this.writeMultiMinSize = writeMultiMinSize;
        this.createDir = createDir;
        this.delete = delete;
        this.copy = copy;
        this.rename = rename;
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
        this.shared = shared;
    }
}
