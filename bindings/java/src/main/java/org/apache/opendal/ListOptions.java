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
public class ListOptions {

    /**
     * Return files in subdirectory as well.
     */
    public final boolean recursive;

    /**
     * The limit passed to underlying service to specify the max results
     * that could return per-request.
     */
    @Builder.Default
    public final long limit = -1;

    /**
     * The startAfter option passes to underlying service to specify the
     * specified key to start listing from.
     */
    public final String startAfter;

    /**
     * The versions option is used to control whether the object versions should be returned.
     */
    public final boolean versions;

    /**
     * The deleted is used to control whether the deleted objects should be returned.
     */
    public final boolean deleted;
}
