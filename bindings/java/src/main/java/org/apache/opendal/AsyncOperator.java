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

import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;

/**
 * AsyncOperator represents an underneath OpenDAL operator that
 * accesses data asynchronously.
 *
 * <p>Operators without an explicit executor share a default executor. Operators,
 * derived streams, and outstanding operations keep it alive. Closing the last
 * operator and stream releases it after outstanding operations complete; a later
 * operator creates a new default executor.</p>
 *
 * <p>The default executor uses the same worker context class loader behavior as {@link AsyncExecutor}.</p>
 */
public class AsyncOperator extends NativeObject {

    /**
     * Singleton to hold all outstanding futures.
     *
     * <p>
     * This is a trick to avoid using global references to pass {@link CompletableFuture}
     * among language boundary and between multiple native threads.
     *
     * @see <a href="https://docs.oracle.com/javase/8/docs/technotes/guides/jni/spec/functions.html#global_references">Global References</a>
     * @see <a href="https://docs.rs/jni/latest/jni/objects/struct.GlobalRef.html">jni::objects::GlobalRef</a>
     */
    private enum AsyncRegistry {
        INSTANCE;

        private final Map<Long, CompletableFuture<?>> registry = new ConcurrentHashMap<>();

        /**
         * Request a new {@link CompletableFuture} that is associated with a unique ID.
         *
         * <p>
         * This method is called from native code. The return ID is used by:
         *
         * <li>Rust side: {@link #get(long)} the future when the native async op completed</li>
         * <li>Java side: {@link #take(long)} the future to compose with more actions</li>
         *
         * @return the request ID associated to the obtained future
         */
        @SuppressWarnings("unused")
        private static long requestId() {
            final CompletableFuture<?> f = new CompletableFuture<>();
            while (true) {
                final long requestId = Math.abs(UUID.randomUUID().getLeastSignificantBits());
                final CompletableFuture<?> prev = INSTANCE.registry.putIfAbsent(requestId, f);
                if (prev == null) {
                    return requestId;
                }
            }
        }

        /**
         * Get the future associated with the request ID.
         *
         * <p>
         * This method is called from native code.
         *
         * @param requestId to identify the future
         * @return the future associated with the request ID
         */
        private static CompletableFuture<?> get(long requestId) {
            return INSTANCE.registry.get(requestId);
        }

        /**
         * Take the future associated with the request ID.
         *
         * @param requestId to identify the future
         * @return the future associated with the request ID
         */
        @SuppressWarnings("unchecked")
        private static <T> CompletableFuture<T> take(long requestId) {
            final CompletableFuture<?> f = get(requestId);
            if (f != null) {
                f.whenComplete((r, e) -> INSTANCE.registry.remove(requestId));
            }
            return (CompletableFuture<T>) f;
        }
    }

    public final OperatorInfo info;

    private final long executorHandle;

    /**
     * Construct an OpenDAL operator.
     *
     * @param config the config of the underneath service to access data from.
     */
    public static AsyncOperator of(ServiceConfig config) {
        return of(config, null);
    }

    /**
     * Construct an OpenDAL operator.
     *
     * @param executor the underneath executor to run async operations; {@code null} to use a default global executor.
     */
    public static AsyncOperator of(ServiceConfig config, AsyncExecutor executor) {
        final String scheme = config.scheme();
        final Map<String, String> map = config.configMap();
        return of(scheme, map, executor);
    }

    /**
     * Construct an OpenDAL operator:
     *
     * <p>
     * You can find all possible schemes <a href="https://docs.rs/opendal/latest/opendal/enum.Scheme.html">here</a>
     * and see what config options each service supports.
     *
     * @param scheme the name of the underneath service to access data from.
     * @param map    a map of properties to construct the underneath operator.
     */
    public static AsyncOperator of(String scheme, Map<String, String> map) {
        return of(scheme, map, null);
    }

    /**
     * Construct an OpenDAL operator:
     *
     * <p>
     * You can find all possible schemes <a href="https://docs.rs/opendal/latest/opendal/enum.Scheme.html">here</a>
     * and see what config options each service supports.
     *
     * @param scheme   the name of the underneath service to access data from.
     * @param map      a map of properties to construct the underneath operator.
     * @param executor the underneath executor to run async operations; {@code null} to use a default global executor.
     */
    public static AsyncOperator of(String scheme, Map<String, String> map, AsyncExecutor executor) {
        final long executorHandle = executor != null ? executor.nativeHandle : 0;
        final long nativeHandle = constructor(scheme, map, executorHandle);
        final OperatorInfo info = makeOperatorInfo(nativeHandle);
        return new AsyncOperator(nativeHandle, executorHandle, info);
    }

    private AsyncOperator(long nativeHandle, long executorHandle, OperatorInfo info) {
        super(nativeHandle);
        this.info = info;
        this.executorHandle = executorHandle;
    }

    /**
     * Clone a new operator that is identical to this one. The new operator has its own lifecycle.
     *
     * <p>Since an operator will release all its resource and "flush" on lifecycle end, this method
     * is suitable to create a narrowed "scope" while avoiding creating a brand-new operator for each
     * scope.
     *
     * @return the cloned operator.
     */
    public AsyncOperator duplicate() {
        final long nativeHandle = duplicate(this.nativeHandle);
        return new AsyncOperator(nativeHandle, this.executorHandle, this.info);
    }

    /**
     * Create a new operator that is layered with the given layer.
     *
     * <p>Note that the current operator is not modified. The returned operator is a new instance.
     * You must close the current operator on demand to avoid resource leak.
     *
     * @param layer the layer to be applied.
     *
     * @return the layered new operator.
     */
    public AsyncOperator layer(Layer layer) {
        final long nativeHandle = layer.layer(this.nativeHandle);
        return new AsyncOperator(nativeHandle, this.executorHandle, makeOperatorInfo(nativeHandle));
    }

    /**
     * Create a new operator that performs every operation blocking.
     *
     * <p>Note that the current operator is not modified. The returned operator is a new instance.
     * You must close the current operator on demand to avoid resource leak.
     *
     * @return the blocking new operator.
     */
    public Operator blocking() {
        final long nativeHandle = makeBlockingOp(this.nativeHandle, this.executorHandle);
        final OperatorInfo info = this.info;
        return new Operator(nativeHandle, info);
    }

    public CompletableFuture<Void> write(String path, String content) {
        return write(
                path,
                content.getBytes(StandardCharsets.UTF_8),
                WriteOptions.builder().build());
    }

    public CompletableFuture<Void> write(String path, byte[] content) {
        return write(path, content, WriteOptions.builder().build());
    }

    public CompletableFuture<Void> write(String path, String content, WriteOptions options) {
        return write(path, content.getBytes(StandardCharsets.UTF_8), options);
    }

    public CompletableFuture<Void> write(String path, byte[] content, WriteOptions options) {
        final long requestId = write(nativeHandle, executorHandle, path, content, options);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<Metadata> stat(String path) {
        return stat(path, StatOptions.builder().build());
    }

    public CompletableFuture<Metadata> stat(String path, StatOptions options) {
        final long requestId = stat(nativeHandle, executorHandle, path, options);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<byte[]> read(String path) {
        return read(path, ReadOptions.builder().build());
    }

    public CompletableFuture<byte[]> read(String path, long offset, long length) {
        return read(path, ReadOptions.builder().offset(offset).length(length).build());
    }

    public CompletableFuture<byte[]> read(String path, ReadOptions options) {
        final long requestId = read(nativeHandle, executorHandle, path, options);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<PresignedRequest> presignRead(String path, Duration duration) {
        return presignRead(path, duration, ReadOptions.builder().build());
    }

    /**
     * Creates a presigned read request with the given options.
     *
     * <p>The service must support presigning this operation. Support for individual options
     * depends on the service. Send the returned HTTP method and headers along with the URI.
     *
     * @param path the path of the object.
     * @param duration how long the presigned request remains valid.
     * @param options the read options to include in the presigned request.
     * @return the presigned request.
     */
    public CompletableFuture<PresignedRequest> presignRead(String path, Duration duration, ReadOptions options) {
        final long requestId = presignRead(nativeHandle, executorHandle, path, duration.toNanos(), options);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<PresignedRequest> presignWrite(String path, Duration duration) {
        return presignWrite(path, duration, WriteOptions.builder().build());
    }

    /**
     * Creates a presigned write request with the given options.
     *
     * <p>The service must support presigning this operation. Support for individual options
     * depends on the service. Send the returned HTTP method and headers along with the URI.
     *
     * @param path the path of the object.
     * @param duration how long the presigned request remains valid.
     * @param options the write options to include in the presigned request.
     * @return the presigned request.
     */
    public CompletableFuture<PresignedRequest> presignWrite(String path, Duration duration, WriteOptions options) {
        final long requestId = presignWrite(nativeHandle, executorHandle, path, duration.toNanos(), options);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<PresignedRequest> presignStat(String path, Duration duration) {
        return presignStat(path, duration, StatOptions.builder().build());
    }

    /**
     * Creates a presigned stat request with the given options.
     *
     * <p>The service must support presigning this operation. Support for individual options
     * depends on the service. Send the returned HTTP method and headers along with the URI.
     *
     * @param path the path of the object.
     * @param duration how long the presigned request remains valid.
     * @param options the stat options to include in the presigned request.
     * @return the presigned request.
     */
    public CompletableFuture<PresignedRequest> presignStat(String path, Duration duration, StatOptions options) {
        final long requestId = presignStat(nativeHandle, executorHandle, path, duration.toNanos(), options);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<Void> delete(String path) {
        final long requestId = delete(nativeHandle, executorHandle, path);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<Void> createDir(String path) {
        final long requestId = createDir(nativeHandle, executorHandle, path);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<Void> copy(String sourcePath, String targetPath) {
        final long requestId = copy(nativeHandle, executorHandle, sourcePath, targetPath);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<Void> rename(String sourcePath, String targetPath) {
        final long requestId = rename(nativeHandle, executorHandle, sourcePath, targetPath);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<Void> removeAll(String path) {
        final long requestId = removeAll(nativeHandle, executorHandle, path);
        return AsyncRegistry.take(requestId);
    }

    public CompletableFuture<List<Entry>> list(String path) {
        return list(path, ListOptions.builder().build());
    }

    public CompletableFuture<List<Entry>> list(String path, ListOptions options) {
        final long requestId = list(nativeHandle, executorHandle, path, options);
        final CompletableFuture<Entry[]> result = AsyncRegistry.take(requestId);
        return Objects.requireNonNull(result).thenApplyAsync(Arrays::asList);
    }

    @Override
    protected native void disposeInternal(long handle);

    private static native long duplicate(long nativeHandle);

    private static native long constructor(String scheme, Map<String, String> map, long executorHandle);

    private static native long read(long nativeHandle, long executorHandle, String path, ReadOptions options);

    private static native long write(
            long nativeHandle, long executorHandle, String path, byte[] content, WriteOptions options);

    private static native long append(long nativeHandle, long executorHandle, String path, byte[] content);

    private static native long delete(long nativeHandle, long executorHandle, String path);

    private static native long stat(long nativeHandle, long executorHandle, String path, StatOptions options);

    private static native long presignRead(
            long nativeHandle, long executorHandle, String path, long duration, ReadOptions options);

    private static native long presignWrite(
            long nativeHandle, long executorHandle, String path, long duration, WriteOptions options);

    private static native long presignStat(
            long nativeHandle, long executorHandle, String path, long duration, StatOptions options);

    private static native OperatorInfo makeOperatorInfo(long nativeHandle);

    private static native long makeBlockingOp(long nativeHandle, long executorHandle);

    private static native long createDir(long nativeHandle, long executorHandle, String path);

    private static native long copy(long nativeHandle, long executorHandle, String sourcePath, String targetPath);

    private static native long rename(long nativeHandle, long executorHandle, String sourcePath, String targetPath);

    private static native long removeAll(long nativeHandle, long executorHandle, String path);

    private static native long list(long nativeHandle, long executorHandle, String path, ListOptions options);
}
