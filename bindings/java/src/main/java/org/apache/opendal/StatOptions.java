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

import java.time.Instant;
import lombok.Builder;
import lombok.Data;

@Data
@Builder
public class StatOptions {

    /**
     * Sets if-match condition for this operation.
     * If file exists and its etag doesn't match, an error will be returned.
     */
    private String ifMatch;

    /**
     * Sets if-none-match condition for this operation.
     * If file exists and its etag matches, an error will be returned.
     */
    private String ifNoneMatch;

    /**
     * Sets if-version-match condition for this operation.
     * If file exists and its version doesn't match, an error will be returned.
     */
    private String ifVersionMatch;

    /**
     * Sets if-version-not-match condition for this operation.
     * If file exists and its version matches, an error will be returned.
     */
    private String ifVersionNotMatch;

    /**
     * Sets if-modified-since condition for this operation.
     * If file exists and hasn't been modified since the specified time, an error will be returned.
     */
    private Instant ifModifiedSince;

    /**
     * Sets if-unmodified-since condition for this operation.
     * If file exists and has been modified since the specified time, an error will be returned.
     */
    private Instant ifUnmodifiedSince;

    /**
     * Sets version for this operation.
     * Retrieves data of a specified version of the given path.
     */
    private String version;

    /**
     * Specifies the content-type header for presigned operations.
     * Only meaningful when used along with presign.
     */
    private String overrideContentType;

    /**
     * Specifies the cache-control header for presigned operations.
     * Only meaningful when used along with presign.
     */
    private String overrideCacheControl;

    /**
     * Specifies the content-disposition header for presigned operations.
     * Only meaningful when used along with presign.
     */
    private String overrideContentDisposition;
}
