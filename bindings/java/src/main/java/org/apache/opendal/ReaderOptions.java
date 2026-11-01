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

/**
 * Selects a file version, read conditions, and execution options for an {@link OperatorReader}.
 * All conditions must hold for data to be returned. Missing files fail with NotFound;
 * failed conditions on existing files fail with ConditionNotMatch. Unsupported conditions
 * fail with Unsupported. Depending on the service, errors can surface when creating the
 * reader or while reading. Conditions apply to every request, including streams.
 * Null version and condition fields leave those options unset.
 *
 * <p>Execution options apply independently of each logical range.
 * Setting concurrent without setting chunk does not enable concurrent range reads.
 * Invalid execution values fail with ConfigInvalid when the reader is created.
 * An Instant outside the Rust core timestamp range fails with Unexpected.
 */
@Builder
public final class ReaderOptions {
    /**
     * Selects a stored file version instead of the current one. This is not a condition.
     * Requires service support for reading versions; a missing version fails with NotFound.
     */
    public final String version;

    /**
     * Reads only when the file has this exact ETag. Requires service support for if-match reads.
     * Only concrete ETags are portable; a wildcard such as "*" has no portable meaning.
     */
    public final String ifMatch;

    /**
     * Reads only when the file exists with a different ETag. Requires service support for
     * if-none-match reads. Only concrete ETags are portable; "*" has no portable meaning.
     */
    public final String ifNoneMatch;

    /**
     * Reads only when the file has this exact version. Requires service support for version-match reads.
     * This checks the selected file's identity rather than selecting a stored version.
     */
    public final String ifVersionMatch;

    /**
     * Reads only when the file exists with a different version.
     * Requires service support for version-not-match reads.
     */
    public final String ifVersionNotMatch;

    /**
     * Reads only when the file was modified after this time.
     * Requires service support for if-modified-since reads.
     */
    public final Instant ifModifiedSince;

    /**
     * Reads only when the file was not modified after this time.
     * Requires service support for if-unmodified-since reads.
     */
    public final Instant ifUnmodifiedSince;

    /**
     * Maximum number of internal chunk requests executed concurrently. Must be positive.
     * This is not the number of application transfers or Java threads and only affects chunked reads.
     */
    @Builder.Default
    public final int concurrent = 1;

    /**
     * Target size of each internal range request, in bytes. A positive value enables chunked reads.
     * The default of -1 keeps unchunked streaming. Zero and other negative values are invalid.
     * The value must fit the native platform's unsigned pointer-sized integer.
     */
    @Builder.Default
    public final long chunk = -1L;

    /**
     * Maximum number of completed chunks buffered ahead of consumption, not a byte count.
     * Must be non-negative. The default of zero applies strict backpressure.
     * This option only affects chunked reads.
     */
    @Builder.Default
    public final int prefetch = 0;

    /**
     * Known full object content length in bytes, independent of the requested range.
     * The default of -1 means unknown; zero is valid for an empty object.
     * This hint can avoid a metadata request and is not a consistency condition.
     * An incorrect hint can cause incomplete reads or errors. Values below -1 are invalid.
     */
    @Builder.Default
    public final long contentLengthHint = -1L;
}
