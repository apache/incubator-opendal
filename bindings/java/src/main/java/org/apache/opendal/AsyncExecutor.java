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

/**
 * AsyncExecutor represents an underneath OpenDAL executor that runs async tasks spawned in the Rust world.
 *
 * <p>If the executor is passed to construct operators, the executor must outlive the operators.</p>
 *
 * <p>Worker threads use the class loader that defines OpenDAL as their context class loader.
 * Completion callbacks that change it must restore it before returning.</p>
 */
public class AsyncExecutor extends NativeObject {
    /**
     * Create a Tokio executor with the specified number of worker threads.
     *
     * @param cores the number of worker threads; must be positive
     * @return the executor
     * @throws IllegalArgumentException if {@code cores} is zero or negative
     */
    public static AsyncExecutor createTokioExecutor(int cores) {
        return new AsyncExecutor(makeTokioExecutor(cores));
    }

    private AsyncExecutor(long nativeHandle) {
        super(nativeHandle);
    }

    @Override
    protected native void disposeInternal(long handle);

    private static native long makeTokioExecutor(int cores);
}
