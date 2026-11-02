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

using OpenDAL.Interop.Result;

namespace OpenDAL;

/// <summary>
/// Read-only stream over an OpenDAL path.
/// </summary>
public sealed class OperatorInputStream : Stream
{
    private IntPtr handle;
    private bool disposed;
    private bool abandoned;

    // Current native chunk. Keeping the native handle instead of a managed
    // copy lets Read copy each byte exactly once, straight into the caller's
    // destination. Released once fully consumed, on EOF, or on dispose.
    private OpenDALReadResult? currentChunk;
    private nuint chunkOffset;

    internal OperatorInputStream(IntPtr handle)
    {
        if (handle == IntPtr.Zero)
        {
            throw new ArgumentException("Native input stream handle must not be zero.", nameof(handle));
        }

        this.handle = handle;
    }

    public override bool CanRead => !disposed && !abandoned;

    public override bool CanSeek => false;

    public override bool CanWrite => false;

    public override long Length => throw new NotSupportedException();

    public override long Position
    {
        get => throw new NotSupportedException();
        set => throw new NotSupportedException();
    }

    public override int Read(byte[] buffer, int offset, int count)
    {
        ArgumentNullException.ThrowIfNull(buffer);
        return Read(buffer.AsSpan(offset, count));
    }

    public override int Read(Span<byte> destination)
    {
        ThrowIfDisposed();

        var totalRead = 0;
        while (destination.Length > 0)
        {
            if (currentChunk is null || chunkOffset >= currentChunk.Value.Buffer.Len)
            {
                ReleaseCurrentChunk();
                if (!AcceptChunk(NativeMethods.operator_input_stream_read_next(handle)))
                {
                    break;
                }
            }

            var copied = CopyFromCurrentChunk(destination);
            if (copied == 0)
            {
                break;
            }

            destination = destination[copied..];
            totalRead += copied;
        }

        return totalRead;
    }

    public override Task<int> ReadAsync(byte[] buffer, int offset, int count, CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(buffer);
        return ReadAsync(buffer.AsMemory(offset, count), cancellationToken).AsTask();
    }

    public override async ValueTask<int> ReadAsync(Memory<byte> destination, CancellationToken cancellationToken = default)
    {
        ThrowIfDisposed();
        cancellationToken.ThrowIfCancellationRequested();

        var totalRead = 0;
        while (!destination.IsEmpty)
        {
            if (currentChunk is null || chunkOffset >= currentChunk.Value.Buffer.Len)
            {
                ReleaseCurrentChunk();
                if (!AcceptChunk(await FetchNextChunkAsync(cancellationToken).ConfigureAwait(false)))
                {
                    break;
                }
            }

            var copied = CopyFromCurrentChunk(destination.Span);
            if (copied == 0)
            {
                break;
            }

            destination = destination[copied..];
            totalRead += copied;
        }

        return totalRead;
    }

    /// <summary>
    /// Awaits the next chunk from the native async read path.
    /// </summary>
    /// <remarks>
    /// On success the returned result still owns its native buffer; the caller
    /// takes that ownership through <see cref="AcceptChunk"/>. When the token
    /// fires first, the late native callback finds no awaiter and releases the
    /// chunk itself, and the stream is poisoned because its position no
    /// longer matches what the caller consumed.
    /// </remarks>
    private async ValueTask<OpenDALReadResult> FetchNextChunkAsync(CancellationToken cancellationToken)
    {
        var context = AsyncStateRegistry.Register<OpenDALReadResult>(out var state);
        OpenDALResult submit;
        unsafe
        {
            submit = NativeMethods.operator_input_stream_read_next_async(
                handle, &Operator.OnReadResultRetained, context);
        }

        try
        {
            Operator.ThrowIfErrorAndRelease(submit);
        }
        catch
        {
            AsyncStateRegistry.Unregister(context);
            throw;
        }

        state.BindCancellation(cancellationToken);
        try
        {
            return await state.Completion.Task.ConfigureAwait(false);
        }
        catch
        {
            // The native position may already sit past a chunk this caller
            // never received. Poison the stream instead of silently skipping
            // those bytes on the next read.
            abandoned = true;
            throw;
        }
    }

    /// <summary>
    /// Takes ownership of a fetched chunk, releasing empty results.
    /// </summary>
    /// <returns><see langword="false"/> on EOF.</returns>
    private bool AcceptChunk(OpenDALReadResult next)
    {
        var error = next.GetError();
        if (error.IsError)
        {
            var exception = new OpenDALException(error);
            next.Release();
            throw exception;
        }

        if (next.Buffer.Handle == IntPtr.Zero || next.Buffer.Len == 0)
        {
            next.Release();
            return false;
        }

        currentChunk = next;
        chunkOffset = 0;
        return true;
    }

    private int CopyFromCurrentChunk(Span<byte> destination)
    {
        var copied = currentChunk!.Value.Buffer.CopyTo(chunkOffset, destination);
        chunkOffset += (nuint)copied;
        return copied;
    }

    private void ReleaseCurrentChunk()
    {
        if (currentChunk is { } chunk)
        {
            chunk.Release();
            currentChunk = null;
            chunkOffset = 0;
        }
    }

    public override void Flush()
    {
    }

    public override Task FlushAsync(CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return Task.CompletedTask;
    }

    public override long Seek(long offset, SeekOrigin origin)
    {
        throw new NotSupportedException();
    }

    public override void SetLength(long value)
    {
        throw new NotSupportedException();
    }

    public override void Write(byte[] buffer, int offset, int count)
    {
        throw new NotSupportedException();
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(disposed || handle == IntPtr.Zero, this);
        if (abandoned)
        {
            throw new InvalidOperationException(
                "An abandoned read left the native stream position unknown; dispose this stream and open a new one.");
        }
    }

    protected override void Dispose(bool disposing)
    {
        if (!disposed)
        {
            ReleaseCurrentChunk();
            NativeMethods.operator_input_stream_free(handle);
            handle = IntPtr.Zero;
            disposed = true;
        }

        base.Dispose(disposing);
    }
}
