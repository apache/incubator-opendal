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

using System.Buffers;
using OpenDAL.Interop.NativeObject;
using OpenDAL.Interop.Result;

namespace OpenDAL.Interop.Buffers;

/// <summary>
/// Native <see cref="IBufferWriter{T}"/> whose contents transfer to the
/// OpenDAL layer when a write consumes them, without copying.
/// </summary>
/// <remarks>
/// Allocate with <see cref="Operator.AllocateWriteBuffer"/>, produce through the
/// standard <see cref="IBufferWriter{T}"/> contract (for example with
/// <c>Utf8JsonWriter</c>), then hand it to a write. The write consumes the
/// buffer: every later access throws. Growth adds native segments that never
/// move, so earlier memory stays valid until <see cref="Dispose"/>. Instances
/// are not thread-safe; disposing an unconsumed buffer returns the memory.
/// </remarks>
internal sealed class WriteBuffer : IBufferWriter<byte>, IDisposable
{
    internal const int DefaultInitialCapacity = 64 * 1024; // 64 KiB

    // New segments double the previous capacity up to this bound, mirroring
    // ArrayBufferWriter's growth curve while keeping segments coarse enough
    // that a large payload stays a handful of chunks.
    private const int MaxSegmentCapacity = 8 * 1024 * 1024; // 8 MiB

    private IntPtr handle;
    private IntPtr data;
    private int capacity;
    private int tailWritten;
    private long totalWritten;
    private bool consumed;
    private NativeMemoryManager? memoryManager;

    internal WriteBuffer(IntPtr handle, IntPtr data, int capacity)
    {
        this.handle = handle;
        this.data = data;
        this.capacity = capacity;
    }

    /// <summary>
    /// Gets the total number of bytes committed with <see cref="Advance"/>.
    /// </summary>
    public long WrittenCount => totalWritten;

    internal IntPtr Handle
    {
        get
        {
            ThrowIfNotUsable();
            return handle;
        }
    }

    internal int TailWritten => tailWritten;

    internal void MarkConsumed()
    {
        consumed = true;
    }

    /// <inheritdoc />
    /// <exception cref="ObjectDisposedException">The buffer has been disposed.</exception>
    /// <exception cref="InvalidOperationException">The buffer contents were already consumed by a write.</exception>
    /// <exception cref="OpenDALException">Growing the buffer fails natively.</exception>
    public unsafe Span<byte> GetSpan(int sizeHint = 0)
    {
        EnsureCapacity(sizeHint);
        return new Span<byte>((byte*)data + tailWritten, capacity - tailWritten);
    }

    /// <inheritdoc />
    /// <exception cref="ObjectDisposedException">The buffer has been disposed.</exception>
    /// <exception cref="InvalidOperationException">The buffer contents were already consumed by a write.</exception>
    /// <exception cref="OpenDALException">Growing the buffer fails natively.</exception>
    public unsafe Memory<byte> GetMemory(int sizeHint = 0)
    {
        EnsureCapacity(sizeHint);
        memoryManager ??= new NativeMemoryManager((byte*)data, capacity);
        return memoryManager.Memory[tailWritten..];
    }

    /// <inheritdoc />
    /// <exception cref="ObjectDisposedException">The buffer has been disposed.</exception>
    /// <exception cref="InvalidOperationException">The buffer contents were already consumed by a write.</exception>
    /// <exception cref="ArgumentOutOfRangeException">
    /// <paramref name="count"/> is negative or exceeds the last acquired span.
    /// </exception>
    public void Advance(int count)
    {
        ThrowIfNotUsable();
        if (count < 0 || count > capacity - tailWritten)
        {
            throw new ArgumentOutOfRangeException(nameof(count), count,
                "Advance count must be between 0 and the size of the last acquired buffer.");
        }

        tailWritten += count;
        totalWritten += count;
    }

    private void EnsureCapacity(int sizeHint)
    {
        ThrowIfNotUsable();
        ArgumentOutOfRangeException.ThrowIfNegative(sizeHint);
        var needed = sizeHint == 0 ? 1 : sizeHint;
        if (capacity - tailWritten >= needed)
        {
            return;
        }

        var nextCapacity = (int)Math.Max(needed, Math.Min((long)capacity * 2, MaxSegmentCapacity));
        var result = NativeMethods.write_buffer_add_segment(handle, (nuint)tailWritten, (nuint)nextCapacity);
        var segment = Operator.ToValueOrThrowAndRelease<OpenDALWriteBuffer, OpenDALWriteResult>(result);

        data = segment.Data;
        capacity = checked((int)segment.Capacity);
        tailWritten = 0;
        // The old manager's memory stays valid because sealed segments never
        // move; the new segment simply needs its own view.
        memoryManager = null;
    }

    private void ThrowIfNotUsable()
    {
        ObjectDisposedException.ThrowIf(handle == IntPtr.Zero, this);
        if (consumed)
        {
            throw new InvalidOperationException(
                "The write buffer contents were already consumed by a write; they now belong to the native layer. Allocate a new buffer for the next write.");
        }
    }

    public void Dispose()
    {
        if (handle != IntPtr.Zero)
        {
            NativeMethods.write_buffer_free(handle);
            handle = IntPtr.Zero;
            data = IntPtr.Zero;
            memoryManager = null;
        }
    }
}
