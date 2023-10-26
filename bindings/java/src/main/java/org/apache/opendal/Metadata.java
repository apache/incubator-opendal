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

import java.util.Date;
import lombok.Data;

/**
 * Metadata carries all metadata associated with a path.
 */
@Data
public class Metadata {
    /**
     * Mode of the entry.
     */
    public final EntryMode mode;
    /**
     * Content Length of the entry.
     *
     * Note: For now, this value is only available when calling on result of `stat`, otherwise it will be -1.
     */
    public final long contentLength;
    /**
     * Content-Disposition of the entry.
     *
     * Note: For now, this value is only available when calling on result of `stat`, otherwise it will be null.
     */
    public final String contentDisposition;
    /**
     * Content MD5 of the entry.
     *
     * Note: For now, this value is only available when calling on result of `stat`, otherwise it will be null.
     */
    public final String contentMd5;
    /**
     * Content Type of the entry.
     *
     * Note: For now, this value is only available when calling on result of `stat`, otherwise it will be null.
     */
    public final String contentType;
    /**
     * Cache Control of the entry.
     *
     * Note: For now, this value is only available when calling on result of `stat`, otherwise it will be null.
     */
    public final String cacheControl;
    /**
     * Etag of the entry.
     *
     * Note: For now, this value is only available when calling on result of `stat`, otherwise it will be null.
     */
    public final String etag;
    /**
     * Last Modified of the entry.
     *
     * Note: For now, this value is only available when calling on result of `stat`, otherwise it will be null.
     */
    public final Date lastModified;
    /**
     * Version of the entry.
     * Version is a string that can be used to identify the version of this entry.
     * This field may come out from the version control system, like object
     * versioning in AWS S3.
     *
     * Note: For now, this value is only available when calling on result of `stat`, otherwise it will be null.
     */
    public final String version;

    public Metadata(
            int mode,
            long contentLength,
            String contentDisposition,
            String contentMd5,
            String contentType,
            String cacheControl,
            String etag,
            Date lastModified,
            String version) {
        this.mode = EntryMode.of(mode);
        this.contentLength = contentLength;
        this.contentDisposition = contentDisposition;
        this.contentMd5 = contentMd5;
        this.contentType = contentType;
        this.cacheControl = cacheControl;
        this.etag = etag;
        this.lastModified = lastModified;
        this.version = version;
    }

    public boolean isFile() {
        return mode == EntryMode.FILE;
    }

    public boolean isDir() {
        return mode == EntryMode.DIR;
    }

    public enum EntryMode {
        /**
         * FILE means the path has data to read.
         */
        FILE,
        /**
         * DIR means the path can be listed.
         */
        DIR,
        /**
         * Unknown means we don't know what we can do on this path.
         */
        UNKNOWN;

        public static EntryMode of(int mode) {
            switch (mode) {
                case 0:
                    return EntryMode.FILE;
                case 1:
                    return EntryMode.DIR;
                default:
                    return EntryMode.UNKNOWN;
            }
        }
    }
}
