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
 * Reads a file synchronously through a reusable Rust core reader.
 * Each read selects its own range and does not advance a shared cursor.
 * Reader options apply to every request, including streams.
 * A reader does not snapshot the file;
 * changes to the file may be visible to subsequent reads.
 *
 * <p>Close the reader when it is no longer needed, preferably with try-with-resources.
 * Closing the operator that created it does not close the reader.
 * Calls on one reader are serialized, including close.
 *
 * @see Operator#createReader(String, ReaderOptions)
 * @see ReaderOptions
 */
public final class OperatorReader extends NativeObject {
    OperatorReader(long nativeHandle) {
        super(nativeHandle);
    }

    /**
     * Reads the whole file into a Java byte array.
     *
     * @return file contents
     * @throws OpenDALException if the file does not exist (NotFound), a condition fails
     *     (ConditionNotMatch), or reading fails
     * @throws IllegalStateException if this reader is closed
     */
    public byte[] read() {
        return read(0, -1);
    }

    /**
     * Reads a byte range into a Java byte array without advancing a shared cursor.
     *
     * @param offset non-negative starting byte offset
     * @param length number of bytes to read, or -1 to read to the end; zero returns an empty array
     * @return contents of the requested range
     * @throws OpenDALException if the range is invalid (RangeNotSatisfied), the file does not
     *     exist (NotFound), a condition fails (ConditionNotMatch), or reading fails
     * @throws IllegalStateException if this reader is closed
     */
    public synchronized byte[] read(long offset, long length) {
        if (isDisposed()) {
            throw new IllegalStateException("OperatorReader is closed");
        }
        return readBytes(nativeHandle, offset, length);
    }

    /**
     * Creates an independent stream over the whole file.
     *
     * @return a stream that the caller must close
     * @see #createInputStream(long, long)
     */
    public OperatorInputStream createInputStream() {
        return createInputStream(0, -1);
    }

    /**
     * Creates a stream over a byte range using this reader's options.
     * Each stream has its own cursor and native resources. Closing this reader does not
     * close its streams, and closing a stream does not close this reader or its other streams.
     *
     * @param offset non-negative starting byte offset
     * @param length number of bytes to read, or -1 to read to the end; zero creates an empty stream
     * @return a stream that the caller must close
     * @throws OpenDALException if the range is invalid (RangeNotSatisfied) or opening the stream fails
     * @throws IllegalStateException if this reader is closed
     */
    public OperatorInputStream createInputStream(long offset, long length) {
        return new OperatorInputStream(createBytesIterator(offset, length));
    }

    // The caller owns the returned iterator independently of this reader.
    synchronized long createBytesIterator(long offset, long length) {
        if (isDisposed()) {
            throw new IllegalStateException("OperatorReader is closed");
        }
        return createBytesIterator(nativeHandle, offset, length);
    }

    /** Releases this reader's native resources. Repeated calls have no effect. */
    @Override
    public synchronized void close() {
        super.close();
    }

    @Override
    protected void disposeInternal(long handle) {
        disposeReader(handle);
    }

    private static native byte[] readBytes(long reader, long offset, long length);

    private static native long createBytesIterator(long reader, long offset, long length);

    private static native void disposeReader(long reader);
}
