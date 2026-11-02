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

using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using OpenDAL.Interop.Result;

namespace OpenDAL;

/// <summary>
/// Write-only stream over an OpenDAL path.
/// </summary>
public sealed class OperatorOutputStream : Stream
{
    internal const int DefaultBufferSize = 16 * 1024; // 16 KiB

    private IntPtr handle;
    private bool disposed;
    private bool completed;
    private bool abandoned;
    private readonly byte[] buffer;
    private int buffered;

    internal OperatorOutputStream(IntPtr handle, int bufferSize)
    {
        if (handle == IntPtr.Zero)
        {
            throw new ArgumentException("Native output stream handle must not be zero.", nameof(handle));
        }

        if (bufferSize <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(bufferSize), "Buffer size must be greater than zero.");
        }

        this.handle = handle;
        buffer = GC.AllocateUninitializedArray<byte>(bufferSize);
    }

    public override bool CanRead => false;

    public override bool CanSeek => false;

    public override bool CanWrite => !disposed && !completed && !abandoned;

    public override long Length => throw new NotSupportedException();

    public override long Position
    {
        get => throw new NotSupportedException();
        set => throw new NotSupportedException();
    }

    public override void Write(byte[] source, int offset, int count)
    {
        ArgumentNullException.ThrowIfNull(source);
        Write(source.AsSpan(offset, count));
    }

    public override void Write(ReadOnlySpan<byte> source)
    {
        ThrowIfDisposed();
        while (!source.IsEmpty)
        {
            var writable = buffer.Length - buffered;
            if (writable == 0)
            {
                FlushBuffered();
                writable = buffer.Length;
            }

            var take = Math.Min(writable, source.Length);
            source[..take].CopyTo(buffer.AsSpan(buffered));
            buffered += take;
            source = source[take..];
        }
    }

    public override Task WriteAsync(byte[] buffer, int offset, int count, CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(buffer);
        return WriteAsync(buffer.AsMemory(offset, count), cancellationToken).AsTask();
    }

    public override async ValueTask WriteAsync(ReadOnlyMemory<byte> source, CancellationToken cancellationToken = default)
    {
        ThrowIfDisposed();
        cancellationToken.ThrowIfCancellationRequested();

        while (!source.IsEmpty)
        {
            var writable = buffer.Length - buffered;
            if (writable == 0)
            {
                await FlushBufferedAsync(cancellationToken).ConfigureAwait(false);
                writable = buffer.Length;
            }

            var take = Math.Min(writable, source.Length);
            source.Span[..take].CopyTo(buffer.AsSpan(buffered));
            buffered += take;
            source = source[take..];
        }
    }

    /// <summary>
    /// Pushes buffered bytes to the native writer.
    /// </summary>
    /// <remarks>
    /// This does not persist the content: the backend finalizes the write
    /// only when the stream completes, through <see cref="Complete"/>,
    /// <see cref="CompleteAsync"/>, or disposal.
    /// </remarks>
    public override void Flush()
    {
        ThrowIfDisposed();
        FlushBuffered();
        var result = NativeMethods.operator_output_stream_flush(handle);
        Operator.ThrowIfErrorAndRelease(result);
    }

    /// <inheritdoc cref="Flush" />
    public override async Task FlushAsync(CancellationToken cancellationToken)
    {
        ThrowIfDisposed();
        cancellationToken.ThrowIfCancellationRequested();
        await FlushBufferedAsync(cancellationToken).ConfigureAwait(false);
        var result = NativeMethods.operator_output_stream_flush(handle);
        Operator.ThrowIfErrorAndRelease(result);
    }

    /// <summary>
    /// Flushes buffered bytes and finalizes the write, persisting the
    /// content.
    /// </summary>
    /// <remarks>
    /// Errors surface here, so prefer completing explicitly over relying on
    /// disposal, which can only close best-effort. After completion the
    /// stream rejects further writes and disposal just releases native
    /// resources.
    /// </remarks>
    /// <exception cref="ObjectDisposedException">The stream has been disposed.</exception>
    /// <exception cref="InvalidOperationException">The stream was already completed.</exception>
    /// <exception cref="OpenDALException">Native close fails.</exception>
    public void Complete()
    {
        ThrowIfDisposed();
        FlushBuffered();
        var result = NativeMethods.operator_output_stream_close(handle);
        Operator.ThrowIfErrorAndRelease(result);
        completed = true;
    }

    /// <inheritdoc cref="Complete" />
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    public async Task CompleteAsync(CancellationToken cancellationToken = default)
    {
        ThrowIfDisposed();
        cancellationToken.ThrowIfCancellationRequested();
        await FlushBufferedAsync(cancellationToken).ConfigureAwait(false);

        var context = AsyncStateRegistry.Register<bool>(out var state);
        OpenDALResult submit;
        unsafe
        {
            submit = NativeMethods.operator_output_stream_close_async(
                handle, &OnStreamWriteCompleted, context);
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
        await state.Completion.Task.ConfigureAwait(false);
        completed = true;
    }

    /// <summary>
    /// Pushes the managed buffer to the native writer without blocking the
    /// calling thread.
    /// </summary>
    private async ValueTask FlushBufferedAsync(CancellationToken cancellationToken)
    {
        if (buffered == 0)
        {
            return;
        }

        var context = AsyncStateRegistry.Register<bool>(out var state);
        OpenDALResult submit;
        unsafe
        {
            submit = NativeMethods.operator_output_stream_write_async(
                handle, buffer, (nuint)buffered, &OnStreamWriteCompleted, context);
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
            await state.Completion.Task.ConfigureAwait(false);
        }
        catch
        {
            // The submitted bytes already belong to the native writer, so the
            // stream cannot tell what landed. Poison it rather than risk
            // flushing the same bytes twice.
            abandoned = true;
            throw;
        }

        buffered = 0;
    }

    /// <summary>
    /// Native callback invoked when an asynchronous stream write finishes.
    /// </summary>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnStreamWriteCompleted(long context, OpenDALResult result)
    {
        Operator.CompleteAsyncCallback(context, result);
    }

    public override int Read(byte[] buffer, int offset, int count)
    {
        throw new NotSupportedException();
    }

    public override long Seek(long offset, SeekOrigin origin)
    {
        throw new NotSupportedException();
    }

    public override void SetLength(long value)
    {
        throw new NotSupportedException();
    }

    private void FlushBuffered()
    {
        if (buffered == 0)
        {
            return;
        }

        var result = NativeMethods.operator_output_stream_write(handle, buffer, (nuint)buffered);
        Operator.ThrowIfErrorAndRelease(result);
        buffered = 0;
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(disposed || handle == IntPtr.Zero, this);
        if (completed)
        {
            throw new InvalidOperationException(
                "The stream was already completed; open a new write stream for further writes.");
        }

        if (abandoned)
        {
            throw new InvalidOperationException(
                "An abandoned flush left the native writer in an unknown state; dispose this stream and open a new one.");
        }
    }

    /// <summary>
    /// Releases the stream. When the stream has not been completed, the
    /// remaining bytes are closed out best-effort and errors are swallowed;
    /// call <see cref="Complete"/> or <see cref="CompleteAsync"/> first when
    /// close errors must surface. A stream poisoned by an abandoned flush is
    /// released without closing.
    /// </summary>
    protected override void Dispose(bool disposing)
    {
        if (!disposed)
        {
            try
            {
                if (!completed && !abandoned)
                {
                    FlushBuffered();
                    var close = NativeMethods.operator_output_stream_close(handle);
                    Operator.ThrowIfErrorAndRelease(close);
                }
            }
            catch
            {
                // Best-effort close only; Complete() is the throwing path.
            }
            finally
            {
                NativeMethods.operator_output_stream_free(handle);
                handle = IntPtr.Zero;
                buffered = 0;
                disposed = true;
            }
        }

        base.Dispose(disposing);
    }

    /// <inheritdoc cref="Dispose(bool)" />
    public override async ValueTask DisposeAsync()
    {
        if (!disposed && !completed && !abandoned)
        {
            try
            {
                await CompleteAsync().ConfigureAwait(false);
            }
            catch
            {
                // Best-effort close only; CompleteAsync() is the throwing path.
            }
        }

        Dispose();
        GC.SuppressFinalize(this);
    }
}
