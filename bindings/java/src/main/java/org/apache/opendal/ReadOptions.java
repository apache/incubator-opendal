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

import lombok.Builder;

@Builder
public class ReadOptions {
    /**
     * Sets the offset of the read operation.
     */
    public final long offset;

    /**
     * Sets the length of the read operation.
     */
    @Builder.Default
    public final long length = -1;

    /**
     * Overrides the Content-Type response header when the service supports it.
     * Also applies to presigned read requests.
     */
    public final String overrideContentType;

    /**
     * Overrides the Cache-Control response header when the service supports it.
     * Also applies to presigned read requests.
     */
    public final String overrideCacheControl;

    /**
     * Overrides the Content-Disposition response header when the service supports it.
     * Use this to set a download filename on a presigned read request.
     */
    public final String overrideContentDisposition;
}
