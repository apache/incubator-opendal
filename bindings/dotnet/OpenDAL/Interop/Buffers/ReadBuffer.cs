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
using System.Runtime.InteropServices;
using OpenDAL.Interop.NativeObject;
using OpenDAL.Interop.Result;

namespace OpenDAL.Interop.Buffers;

/// <summary>
/// Read payload kept in native memory and exposed as a
/// <see cref="ReadOnlySequence{T}"/>, without copying.
/// </summary>
/// <remarks>
/// Returned by <see cref="Operator.ReadBuffer"/> and
/// <see cref="Operator.ReadBufferAsync"/>; consumers such as
/// <c>Utf8JsonReader</c> parse straight over native memory. The memory stays
/// valid until <see cref="Dispose"/> and must not be used afterwards.
/// Instances are not thread-safe.
/// </remarks>
internal sealed class ReadBuffer : IDisposable
{
    private OpenDALReadResult result;
    private ReadOnlySequence<byte> sequence;
    private bool disposed;

    private ReadBuffer(OpenDALReadResult result)
    {
        this.result = result;
        Length = (long)result.Buffer.Len;
        sequence = BuildSequence(result.Buffer);
    }

    /// <summary>
    /// Gets the payload length in bytes.
    /// </summary>
    public long Length { get; }

    /// <summary>
    /// Gets the payload as a read-only sequence over native memory.
    /// </summary>
    /// <exception cref="ObjectDisposedException">The buffer has been disposed.</exception>
    public ReadOnlySequence<byte> Sequence
    {
        get
        {
            ObjectDisposedException.ThrowIf(disposed, this);
            return sequence;
        }
    }

    /// <summary>
    /// Takes ownership of a successful native read result; throws and
    /// releases it on error.
    /// </summary>
    internal static ReadBuffer FromResult(OpenDALReadResult result)
    {
        var error = result.GetError();
        if (error.IsError)
        {
            var exception = new OpenDALException(error);
            result.Release();
            throw exception;
        }

        try
        {
            return new ReadBuffer(result);
        }
        catch
        {
            result.Release();
            throw;
        }
    }

    private static unsafe ReadOnlySequence<byte> BuildSequence(OpenDALReadBuffer buffer)
    {
        if (buffer.Handle == IntPtr.Zero || buffer.Len == 0)
        {
            return ReadOnlySequence<byte>.Empty;
        }

        var count = (int)NativeMethods.read_buffer_chunks(buffer.Handle, null, 0);
        if (count == 0)
        {
            return ReadOnlySequence<byte>.Empty;
        }

        var chunks = new OpenDALChunk[count];
        fixed (OpenDALChunk* chunksPtr = chunks)
        {
            _ = NativeMethods.read_buffer_chunks(buffer.Handle, chunksPtr, (nuint)count);
        }

        if (count == 1)
        {
            var memory = new NativeMemoryManager((byte*)chunks[0].Data, checked((int)chunks[0].Len)).Memory;
            return new ReadOnlySequence<byte>(memory);
        }

        ChunkSegment? first = null;
        ChunkSegment? previous = null;
        long runningIndex = 0;
        foreach (var chunk in chunks)
        {
            var memory = new NativeMemoryManager((byte*)chunk.Data, checked((int)chunk.Len)).Memory;
            var segment = new ChunkSegment(memory, runningIndex);
            runningIndex += memory.Length;

            first ??= segment;
            previous?.SetNext(segment);
            previous = segment;
        }

        return new ReadOnlySequence<byte>(first!, 0, previous!, previous!.Memory.Length);
    }

    public void Dispose()
    {
        if (!disposed)
        {
            disposed = true;
            sequence = ReadOnlySequence<byte>.Empty;
            result.Release();
            result = default;
        }
    }

    /// <summary>
    /// Links one native chunk into the sequence.
    /// </summary>
    private sealed class ChunkSegment : ReadOnlySequenceSegment<byte>
    {
        public ChunkSegment(Memory<byte> memory, long runningIndex)
        {
            Memory = memory;
            RunningIndex = runningIndex;
        }

        public void SetNext(ChunkSegment next)
        {
            Next = next;
        }
    }
}

[StructLayout(LayoutKind.Sequential)]
/// <summary>
/// Descriptor of one contiguous chunk inside a native read payload.
/// <see cref="Data"/> stays valid until the owning buffer handle is released.
/// </summary>
internal struct OpenDALChunk
{
    public IntPtr Data;

    public nuint Len;
}
