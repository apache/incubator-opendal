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
using System.Runtime.CompilerServices;
using OpenDAL.Interop.Buffers;
using OpenDAL.Interop.NativeObject;
using OpenDAL.Interop.Result;
using OpenDAL.Interop.Result.Abstractions;
using OpenDAL.Layer.Abstractions;
using OpenDAL.Options;
using OpenDAL.Options.Abstractions;
using OpenDAL.ServiceConfig.Abstractions;

namespace OpenDAL;

/// <summary>
/// Managed wrapper over an OpenDAL native operator handle.
/// </summary>
public partial class Operator : SafeHandle
{
    private readonly OperatorInfo info;

    private Operator(IntPtr nativeHandle) : base(IntPtr.Zero, true)
    {
        if (nativeHandle == IntPtr.Zero)
        {
            throw new ArgumentException("Native operator handle must not be zero.", nameof(nativeHandle));
        }

        SetHandle(nativeHandle);
        info = CreateOperatorInfo();
    }

    /// <summary>
    /// Gets metadata of this operator.
    /// </summary>
    /// <exception cref="ObjectDisposedException">The operator has been disposed.</exception>
    /// <exception cref="OpenDALException">Native operator info retrieval fails.</exception>
    public OperatorInfo Info
    {
        get
        {
            ObjectDisposedException.ThrowIf(IsInvalid, this);
            return info;
        }
    }

    /// <summary>
    /// Gets whether the native handle is invalid.
    /// </summary>
    public override bool IsInvalid => handle == IntPtr.Zero;

    /// <summary>
    /// Creates an operator for the specified backend scheme and options.
    /// </summary>
    /// <remarks>
    /// Available scheme names are defined by OpenDAL.
    /// See <see href="https://docs.rs/opendal/latest/opendal/enum.Scheme.html">OpenDAL Scheme documentation</see>
    /// for supported backends and their related configuration options.
    /// </remarks>
    /// <param name="scheme">Name of the backend service, such as <c>fs</c> or <c>memory</c>.</param>
    /// <param name="options">Key/value options used to configure the selected backend service.</param>
    /// <exception cref="ArgumentException"><paramref name="scheme"/> is null, empty, or whitespace.</exception>
    /// <exception cref="ObjectDisposedException"><paramref name="executor"/> has been disposed.</exception>
    /// <exception cref="OpenDALException">Native operator construction fails.</exception>
    public Operator(
        string scheme,
        IReadOnlyDictionary<string, string>? options = null,
        Executor? executor = null) : base(IntPtr.Zero, true)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(scheme);
        SetHandle(Construct(scheme, options, executor));
        info = CreateOperatorInfo();
    }

    /// <summary>
    /// Creates an operator from a typed service configuration.
    /// </summary>
    /// <remarks>
    /// This overload converts <paramref name="config"/> into backend key/value options internally,
    /// then creates the same native operator as <see cref="Operator(string, IReadOnlyDictionary{string, string}?)"/>.
    /// </remarks>
    /// <param name="config">Typed service configuration for the target backend service.</param>
    /// <exception cref="ArgumentNullException"><paramref name="config"/> is null.</exception>
    /// <exception cref="OpenDALException">Native operator construction fails.</exception>
    public Operator(IServiceConfig config, Executor? executor = null) : base(IntPtr.Zero, true)
    {
        ArgumentNullException.ThrowIfNull(config);
        SetHandle(Construct(config.Scheme, config.ToOptions(), executor));
        info = CreateOperatorInfo();
    }

    private static IntPtr Construct(
        string scheme,
        IReadOnlyDictionary<string, string>? options,
        Executor? executor)
    {
        using var nativeOptionsHandle = CreateConstructorOptionsHandle(options);
        var executorAddRefed = false;
        try
        {
            executor?.DangerousAddRef(ref executorAddRefed);
            var result = NativeMethods.operator_construct(
                scheme,
                GetOptionsHandle(nativeOptionsHandle),
                executor?.DangerousGetHandle() ?? IntPtr.Zero);
            return ToValueOrThrowAndRelease<IntPtr, OpenDALOperatorResult>(result);
        }
        finally
        {
            if (executorAddRefed)
            {
                executor!.DangerousRelease();
            }
        }
    }

    /// <summary>
    /// Applies the specified layer and returns a new operator instance.
    /// </summary>
    /// <param name="layer">Layer to apply.</param>
    /// <returns>A new operator with the layer applied.</returns>
    /// <exception cref="ArgumentNullException"><paramref name="layer"/> is null.</exception>
    /// <exception cref="ObjectDisposedException">The operator has been disposed.</exception>
    /// <exception cref="OpenDALException">Native layer application fails.</exception>
    public Operator WithLayer(ILayer layer)
    {
        ArgumentNullException.ThrowIfNull(layer);
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        return layer.Apply(this);
    }

    /// <summary>
    /// Writes the specified content to a path with write options.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="content">Bytes to write.</param>
    /// <param name="options">Additional write options.</param>
    /// <returns>Metadata of the written object.</returns>
    public Metadata Write(string path, byte[] content, WriteOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(content);
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        using var nativeOptionsHandle = options?.BuildNativeOptionsHandle();
        var result = NativeMethods.operator_write_bytes_with_options(
            this,
            path,
            content,
            (nuint)content.Length,
            GetOptionsHandle(nativeOptionsHandle)
        );

        return ToValueOrThrowAndRelease<Metadata, OpenDALMetadataResult>(result);
    }

    /// <summary>
    /// Writes a payload produced directly into native memory, without a
    /// managed copy.
    /// </summary>
    /// <remarks>
    /// <paramref name="fill"/> runs synchronously against a native-backed
    /// <see cref="IBufferWriter{T}"/>, so standard producers such as
    /// <c>System.Text.Json.Utf8JsonWriter</c> serialize straight into the
    /// write payload. Everything committed when it returns becomes the
    /// payload of a single write. The buffer never leaves this call: it
    /// grows on demand while filling and is released once the backend
    /// finishes with the payload.
    /// </remarks>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="fill">Producer invoked once to fill the payload.</param>
    /// <param name="sizeHint">
    /// Expected payload size in bytes, or 0 for the default. A positive hint
    /// sizes the first native segment so a well-estimated payload fills it
    /// without growing; it is an estimate, not a limit.
    /// </param>
    /// <param name="options">Additional write options, or <see langword="null"/> for default behavior.</param>
    /// <returns>Metadata of the written object.</returns>
    /// <exception cref="ArgumentNullException"><paramref name="fill"/> is null.</exception>
    /// <exception cref="ArgumentOutOfRangeException"><paramref name="sizeHint"/> is negative.</exception>
    /// <exception cref="ObjectDisposedException">The operator or executor has been disposed.</exception>
    /// <exception cref="OpenDALException">Native write fails.</exception>
    public Metadata Write(
        string path,
        Action<IBufferWriter<byte>> fill,
        int sizeHint = 0,
        WriteOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(fill);
        ArgumentOutOfRangeException.ThrowIfNegative(sizeHint);

        using var buffer = AllocateWriteBuffer(
            sizeHint > 0 ? sizeHint : WriteBuffer.DefaultInitialCapacity);
        fill(buffer);
        return Write(path, buffer, options);
    }

    /// <summary>
    /// Writes the bytes committed to an allocated native buffer to a path
    /// synchronously, transferring ownership of its contents to the native
    /// layer without copying.
    /// </summary>
    /// <remarks>
    /// On success the write consumes the buffer contents: every later access
    /// throws, and the memory is released once the buffer is disposed and
    /// the backend finishes with the payload. An error raised before the
    /// payload is taken leaves the buffer usable, but a backend failure
    /// after that point has already consumed it, so treat a failed buffer
    /// as spent.
    /// </remarks>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="content">Allocated buffer holding the bytes to write.</param>
    /// <param name="options">Additional write options, or <see langword="null"/> for default behavior.</param>
    /// <returns>Metadata of the written object.</returns>
    /// <exception cref="ObjectDisposedException">The operator or buffer has been disposed.</exception>
    /// <exception cref="InvalidOperationException">The buffer contents were already consumed by a write.</exception>
    /// <exception cref="OpenDALException">Native write fails.</exception>
    internal Metadata Write(string path, WriteBuffer content, WriteOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(content);
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        var bufferHandle = content.Handle;
        var committedInTail = content.TailWritten;

        using var nativeOptionsHandle = options?.BuildNativeOptionsHandle();
        var result = NativeMethods.operator_write_with_options(
            this,
            path,
            bufferHandle,
            (nuint)committedInTail,
            GetOptionsHandle(nativeOptionsHandle)
        );

        if (!result.Error.IsError)
        {
            content.MarkConsumed();
        }

        return ToValueOrThrowAndRelease<Metadata, OpenDALMetadataResult>(result);
    }

    /// <summary>
    /// Writes the specified content to a path asynchronously with optional write options and executor.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="content">Bytes to write.</param>
    /// <param name="options">Additional write options, or <see langword="null"/> for default behavior.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that resolves with the metadata of the written object.</returns>
    public Task<Metadata> WriteAsync(
        string path,
        byte[] content,
        WriteOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(content);
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        return SubmitAsyncOperation<Metadata, WriteOptions>(options, DispatchWriteBytesAsync, cancellationToken);

        OpenDALResult DispatchWriteBytesAsync(long context, IntPtr optionsHandle)
        {
            unsafe
            {
                return NativeMethods.operator_write_bytes_with_options_async(
                    this,
                    path,
                    content,
                    (nuint)content.Length,
                    optionsHandle,
                    &OnWriteCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Allocates a native <see cref="System.Buffers.IBufferWriter{T}"/> for a
    /// zero-copy write.
    /// </summary>
    /// <remarks>
    /// Produce into the buffer through the <see cref="System.Buffers.IBufferWriter{T}"/>
    /// contract — for example with <c>System.Text.Json.Utf8JsonWriter</c> —
    /// then hand it to <c>Write(path, buffer)</c> or
    /// <c>WriteAsync(path, buffer)</c>. The buffer grows on demand, so
    /// <paramref name="initialCapacity"/> is a hint, not a limit. The write
    /// consumes the buffer; allocate a new one for the next write. Dispose
    /// an unconsumed buffer to return the memory.
    /// </remarks>
    /// <param name="initialCapacity">Size of the first segment in bytes.</param>
    /// <returns>An empty native buffer writer.</returns>
    /// <exception cref="ObjectDisposedException">The operator has been disposed.</exception>
    /// <exception cref="ArgumentOutOfRangeException"><paramref name="initialCapacity"/> is not positive.</exception>
    /// <exception cref="OpenDALException">Native buffer allocation fails.</exception>
    internal WriteBuffer AllocateWriteBuffer(int initialCapacity = WriteBuffer.DefaultInitialCapacity)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(initialCapacity);

        var result = NativeMethods.write_buffer_create((nuint)initialCapacity);
        var buffer = ToValueOrThrowAndRelease<OpenDALWriteBuffer, OpenDALWriteResult>(result);
        return new WriteBuffer(buffer.Handle, buffer.Data, checked((int)buffer.Capacity));
    }

    /// <summary>
    /// Writes the bytes committed to an allocated native buffer to a path
    /// asynchronously, transferring ownership of its contents to the native
    /// layer without copying.
    /// </summary>
    /// <remarks>
    /// Everything committed with <see cref="WriteBuffer.Advance"/> is
    /// written, in order. On success the buffer is consumed: every later
    /// access to it throws, and its memory is released once the buffer is
    /// disposed and the backend finishes with the payload. When dispatch
    /// fails immediately, ownership does not transfer and the buffer stays
    /// usable.
    /// </remarks>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="content">Allocated buffer holding the bytes to write.</param>
    /// <param name="options">Additional write options, or <see langword="null"/> for default behavior.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that resolves with the metadata of the written object.</returns>
    /// <exception cref="ObjectDisposedException">The operator or buffer has been disposed.</exception>
    /// <exception cref="InvalidOperationException">The buffer contents were already consumed by a write.</exception>
    /// <exception cref="OperationCanceledException"><paramref name="cancellationToken"/> is already canceled.</exception>
    /// <exception cref="OpenDALException">Native write submission fails immediately.</exception>
    internal Task<Metadata> WriteAsync(
        string path,
        WriteBuffer content,
        WriteOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(content);
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        // Surfaces disposed/already-consumed as managed exceptions before the
        // native call; the native slot enforces the same contract again.
        var bufferHandle = content.Handle;
        var committedInTail = content.TailWritten;

        return SubmitAsyncOperation<Metadata, WriteOptions>(options, DispatchWriteBufferAsync, cancellationToken);

        OpenDALResult DispatchWriteBufferAsync(long context, IntPtr optionsHandle)
        {
            unsafe
            {
                var result = NativeMethods.operator_write_with_options_async(
                    this,
                    path,
                    bufferHandle,
                    (nuint)committedInTail,
                    optionsHandle,
                    &OnWriteCompleted,
                    context
                );

                if (!result.Error.IsError)
                {
                    content.MarkConsumed();
                }

                return result;
            }
        }
    }

    /// <summary>
    /// Writes a payload produced directly into native memory, without a
    /// managed copy.
    /// </summary>
    /// <remarks>
    /// <paramref name="fill"/> runs synchronously against a native-backed
    /// <see cref="IBufferWriter{T}"/>, so standard producers such as
    /// <c>System.Text.Json.Utf8JsonWriter</c> serialize straight into the
    /// write payload. Everything committed when it returns becomes the
    /// payload of a single write. The buffer never leaves this call: it
    /// grows on demand while filling and is released once the backend
    /// finishes with the payload.
    /// </remarks>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="fill">Producer invoked once to fill the payload.</param>
    /// <param name="sizeHint">
    /// Expected payload size in bytes, or 0 for the default. A positive hint
    /// sizes the first native segment so a well-estimated payload fills it
    /// without growing; it is an estimate, not a limit.
    /// </param>
    /// <param name="options">Additional write options, or <see langword="null"/> for default behavior.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that resolves with the metadata of the written object.</returns>
    /// <exception cref="ArgumentNullException"><paramref name="fill"/> is null.</exception>
    /// <exception cref="ArgumentOutOfRangeException"><paramref name="sizeHint"/> is negative.</exception>
    /// <exception cref="ObjectDisposedException">The operator or executor has been disposed.</exception>
    /// <exception cref="OperationCanceledException"><paramref name="cancellationToken"/> is already canceled.</exception>
    /// <exception cref="OpenDALException">Native write submission fails immediately.</exception>
    public async Task<Metadata> WriteAsync(
        string path,
        Action<IBufferWriter<byte>> fill,
        int sizeHint = 0,
        WriteOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(fill);
        ArgumentOutOfRangeException.ThrowIfNegative(sizeHint);

        using var buffer = AllocateWriteBuffer(
            sizeHint > 0 ? sizeHint : WriteBuffer.DefaultInitialCapacity);
        fill(buffer);
        return await WriteAsync(path, buffer, options, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Reads bytes from a path with read options.
    /// </summary>
    /// <param name="path">Source path in the configured backend.</param>
    /// <param name="options">Additional read options.</param>
    /// <returns>The content bytes.</returns>
    public byte[] Read(string path, ReadOptions? options = null)
    {
        return Read(path, static sequence => sequence.ToArray(), options);
    }

    /// <summary>
    /// Reads bytes from a path asynchronously with optional read options and executor.
    /// </summary>
    /// <param name="path">Source path in the configured backend.</param>
    /// <param name="options">Additional read options, or <see langword="null"/> for default behavior.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that resolves with the read content.</returns>
    public Task<byte[]> ReadAsync(
        string path,
        ReadOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        return ReadAsync(path, static sequence => sequence.ToArray(), options, cancellationToken);
    }

    /// <summary>
    /// Reads a path into a native buffer exposed as a
    /// <see cref="System.Buffers.ReadOnlySequence{T}"/>, without copying into
    /// a managed array.
    /// </summary>
    /// <remarks>
    /// Dispose the returned buffer to release the native memory; the sequence
    /// must not be used afterwards.
    /// </remarks>
    /// <param name="path">Source path in the configured backend.</param>
    /// <param name="options">Additional read options, or <see langword="null"/> for default behavior.</param>
    /// <returns>The content as a disposable native buffer.</returns>
    /// <exception cref="ObjectDisposedException">The operator or executor has been disposed.</exception>
    /// <exception cref="OpenDALException">Native read fails.</exception>
    internal ReadBuffer ReadBuffer(string path, ReadOptions? options = null)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        using var nativeOptionsHandle = options?.BuildNativeOptionsHandle();
        var result = NativeMethods.operator_read_with_options(
            this, path, GetOptionsHandle(nativeOptionsHandle));

        return Interop.Buffers.ReadBuffer.FromResult(result);
    }

    /// <summary>
    /// Reads a path asynchronously into a native buffer exposed as a
    /// <see cref="System.Buffers.ReadOnlySequence{T}"/>, without copying into
    /// a managed array.
    /// </summary>
    /// <remarks>
    /// Dispose the returned buffer to release the native memory; the sequence
    /// must not be used afterwards.
    /// </remarks>
    /// <param name="path">Source path in the configured backend.</param>
    /// <param name="options">Additional read options, or <see langword="null"/> for default behavior.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that resolves with the content as a disposable native buffer.</returns>
    /// <exception cref="ObjectDisposedException">The operator or executor has been disposed.</exception>
    /// <exception cref="OperationCanceledException"><paramref name="cancellationToken"/> is already canceled.</exception>
    /// <exception cref="OpenDALException">Native read submission fails immediately.</exception>
    internal async Task<ReadBuffer> ReadBufferAsync(
        string path,
        ReadOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        cancellationToken.ThrowIfCancellationRequested();

        var context = AsyncStateRegistry.Register<OpenDALReadResult>(out var state);
        OpenDALResult submit;
        using (var nativeOptionsHandle = options?.BuildNativeOptionsHandle())
        {
            unsafe
            {
                submit = NativeMethods.operator_read_with_options_async(
                    this,
                    path,
                    GetOptionsHandle(nativeOptionsHandle),
                    &OnReadResultRetained,
                    context
                );
            }
        }

        try
        {
            ThrowIfErrorAndRelease(submit);
        }
        catch
        {
            AsyncStateRegistry.Unregister(context);
            throw;
        }

        state.BindCancellation(cancellationToken);
        var result = await state.Completion.Task.ConfigureAwait(false);
        return Interop.Buffers.ReadBuffer.FromResult(result);
    }

    /// <summary>
    /// Reads a path and consumes the payload in place as a
    /// <see cref="ReadOnlySequence{T}"/> over native memory, without copying
    /// into a managed array.
    /// </summary>
    /// <remarks>
    /// <paramref name="consume"/> runs synchronously, so standard consumers
    /// such as <c>System.Text.Json.Utf8JsonReader</c> parse straight from
    /// native memory. The memory is released when it returns: the sequence,
    /// and anything pointing into it, must not escape the callback.
    /// </remarks>
    /// <typeparam name="T">Result produced by the consumer.</typeparam>
    /// <param name="path">Source path in the configured backend.</param>
    /// <param name="consume">Consumer invoked once with the payload.</param>
    /// <param name="options">Additional read options, or <see langword="null"/> for default behavior.</param>
    /// <returns>The value returned by <paramref name="consume"/>.</returns>
    /// <exception cref="ArgumentNullException"><paramref name="consume"/> is null.</exception>
    /// <exception cref="ObjectDisposedException">The operator or executor has been disposed.</exception>
    /// <exception cref="OpenDALException">Native read fails.</exception>
    public T Read<T>(
        string path,
        Func<ReadOnlySequence<byte>, T> consume,
        ReadOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(consume);

        using var buffer = ReadBuffer(path, options);
        return consume(buffer.Sequence);
    }

    /// <inheritdoc cref="Read{T}(string, Func{ReadOnlySequence{byte}, T}, ReadOptions?, Executor?)" />
    /// <summary>
    /// Reads a path asynchronously and consumes the payload in place as a
    /// <see cref="ReadOnlySequence{T}"/> over native memory, without copying
    /// into a managed array.
    /// </summary>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <exception cref="OperationCanceledException"><paramref name="cancellationToken"/> is already canceled.</exception>
    public async Task<T> ReadAsync<T>(
        string path,
        Func<ReadOnlySequence<byte>, T> consume,
        ReadOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(consume);

        using var buffer = await ReadBufferAsync(path, options, cancellationToken).ConfigureAwait(false);
        return consume(buffer.Sequence);
    }

    /// <summary>
    /// Native read callback that keeps ownership of a successful payload with
    /// the awaiter instead of materializing it. Without an awaiter, for
    /// example after cancellation, the payload is released here.
    /// </summary>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    internal static void OnReadResultRetained(long context, OpenDALReadResult result)
    {
        if (!AsyncStateRegistry.TryTake<AsyncState<OpenDALReadResult>>(context, out var state))
        {
            result.Release();
            return;
        }

        state.CancellationRegistration.Dispose();
        if (!state.Completion.TrySetResult(result))
        {
            // Cancellation won the completion race, so no awaiter will ever
            // take ownership of this payload.
            result.Release();
        }
    }

    /// <summary>
    /// Gets metadata for the specified path.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="options">Additional stat options.</param>
    /// <returns>Metadata of the target path.</returns>
    public Metadata Stat(string path, StatOptions? options = null)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        OpenDALMetadataResult result;
        using var nativeOptionsHandle = options?.BuildNativeOptionsHandle();
        result = NativeMethods.operator_stat_with_options(this, path, GetOptionsHandle(nativeOptionsHandle));

        return ToValueOrThrowAndRelease<Metadata, OpenDALMetadataResult>(result);
    }

    /// <summary>
    /// Gets metadata for the specified path asynchronously.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="options">Additional stat options.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that resolves with metadata.</returns>
    public Task<Metadata> StatAsync(
        string path,
        StatOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        return SubmitAsyncOperation<Metadata, StatOptions>(options, SubmitStatAsync, cancellationToken);

        OpenDALResult SubmitStatAsync(long context, IntPtr optionsHandle)
        {
            unsafe
            {
                return NativeMethods.operator_stat_with_options_async(
                    this,
                    path,
                    optionsHandle,
                    &OnStatCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Checks whether the specified path exists.
    /// </summary>
    /// <remarks>
    /// A <c>NotFound</c> error from the backend yields <see langword="false"/>;
    /// any other error is thrown.
    /// </remarks>
    /// <param name="path">Target path in the configured backend.</param>
    /// <returns><see langword="true"/> when the path exists.</returns>
    public bool Exists(string path)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        var result = NativeMethods.operator_exists(this, path);
        return ToValueOrThrowAndRelease<bool, OpenDALBoolResult>(result);
    }

    /// <summary>
    /// Checks whether the specified path exists asynchronously.
    /// </summary>
    /// <remarks>
    /// A <c>NotFound</c> error from the backend yields <see langword="false"/>;
    /// any other error is thrown.
    /// </remarks>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that resolves with <see langword="true"/> when the path exists.</returns>
    public Task<bool> ExistsAsync(string path, CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        return SubmitAsyncOperation<bool>(SubmitExistsAsync, cancellationToken);

        OpenDALResult SubmitExistsAsync(long context)
        {
            unsafe
            {
                return NativeMethods.operator_exists_async(
                    this,
                    path,
                    &OnExistsCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Checks whether the operator can reach its service.
    /// </summary>
    /// <remarks>
    /// Lists the root with a limit of one entry. A <c>NotFound</c> error is
    /// treated as success; any other error is thrown.
    /// </remarks>
    public void Check()
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        var result = NativeMethods.operator_check(this);
        ThrowIfErrorAndRelease(result);
    }

    /// <summary>
    /// Checks whether the operator can reach its service asynchronously.
    /// </summary>
    /// <remarks>
    /// Lists the root with a limit of one entry. A <c>NotFound</c> error is
    /// treated as success; any other error is thrown.
    /// </remarks>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that completes when the native callback reports completion.</returns>
    public Task CheckAsync(CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        return SubmitAsyncOperation(SubmitCheckAsync, cancellationToken);

        OpenDALResult SubmitCheckAsync(long context)
        {
            unsafe
            {
                return NativeMethods.operator_check_async(
                    this,
                    &OnCheckCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Lists entries under the specified path.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="options">Additional list options.</param>
    /// <returns>Listed entries.</returns>
    public IReadOnlyList<Entry> List(string path, ListOptions? options = null)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        OpenDALEntryListResult result;
        using var nativeOptionsHandle = options?.BuildNativeOptionsHandle();
        result = NativeMethods.operator_list_with_options(this, path, GetOptionsHandle(nativeOptionsHandle));

        return ToValueOrThrowAndRelease<IReadOnlyList<Entry>, OpenDALEntryListResult>(result);
    }

    /// <summary>
    /// Lists entries under the specified path asynchronously.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="options">Additional list options.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that resolves with listed entries.</returns>
    public Task<IReadOnlyList<Entry>> ListAsync(
        string path,
        ListOptions? options = null,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        return SubmitAsyncOperation<IReadOnlyList<Entry>, ListOptions>(options, SubmitListAsync, cancellationToken);

        OpenDALResult SubmitListAsync(long context, IntPtr optionsHandle)
        {
            unsafe
            {
                return NativeMethods.operator_list_with_options_async(
                    this,
                    path,
                    optionsHandle,
                    &OnListCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Duplicates this operator and returns a new managed instance.
    /// </summary>
    /// <returns>A new operator handle that shares the same backend configuration.</returns>
    /// <exception cref="ObjectDisposedException">The operator has been disposed.</exception>
    /// <exception cref="OpenDALException">Native operator duplication fails.</exception>
    public Operator Duplicate()
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        var result = NativeMethods.operator_duplicate(this);
        var newHandle = ToValueOrThrowAndRelease<IntPtr, OpenDALOperatorResult>(result);
        if (newHandle == IntPtr.Zero)
        {
            throw new InvalidOperationException("Duplicate returned null operator pointer");
        }

        return new Operator(newHandle);
    }

    /// <summary>
    /// Deletes the file at the specified path.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="options">Additional delete options.</param>
    public void Delete(string path, DeleteOptions? options = null)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        using var nativeOptionsHandle = options?.BuildNativeOptionsHandle();
        var result = NativeMethods.operator_delete_with_options(this, path, GetOptionsHandle(nativeOptionsHandle));
        ThrowIfErrorAndRelease(result);
    }

    /// <summary>
    /// Deletes the file at the specified path asynchronously.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="options">Additional delete options.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that completes when the native callback reports completion.</returns>
    public Task DeleteAsync(string path, DeleteOptions? options = null, CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        return SubmitAsyncOperation(options, SubmitDeleteAsync, cancellationToken);

        OpenDALResult SubmitDeleteAsync(long context, IntPtr optionsHandle)
        {
            unsafe
            {
                return NativeMethods.operator_delete_with_options_async(
                    this,
                    path,
                    optionsHandle,
                    &OnDeleteCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Creates a directory at the specified path.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    public void CreateDir(string path)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        var result = NativeMethods.operator_create_dir(this, path);
        ThrowIfErrorAndRelease(result);
    }

    /// <summary>
    /// Creates a directory at the specified path asynchronously.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that completes when the native callback reports completion.</returns>
    public Task CreateDirAsync(string path, CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        return SubmitAsyncOperation(SubmitCreateDirAsync, cancellationToken);

        OpenDALResult SubmitCreateDirAsync(long context)
        {
            unsafe
            {
                return NativeMethods.operator_create_dir_async(
                    this,
                    path,
                    &OnCreateDirCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Copies a file from source path to target path.
    /// </summary>
    /// <param name="sourcePath">Source path in the configured backend.</param>
    /// <param name="targetPath">Target path in the configured backend.</param>
    /// <returns>Metadata of the copied object at <paramref name="targetPath"/>.</returns>
    public Metadata Copy(string sourcePath, string targetPath)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        var result = NativeMethods.operator_copy(this, sourcePath, targetPath);
        return ToValueOrThrowAndRelease<Metadata, OpenDALMetadataResult>(result);
    }

    /// <summary>
    /// Copies a file from source path to target path asynchronously.
    /// </summary>
    /// <param name="sourcePath">Source path in the configured backend.</param>
    /// <param name="targetPath">Target path in the configured backend.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that resolves with the metadata of the copied object.</returns>
    public Task<Metadata> CopyAsync(
        string sourcePath,
        string targetPath,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        return SubmitAsyncOperation<Metadata>(SubmitCopyAsync, cancellationToken);

        OpenDALResult SubmitCopyAsync(long context)
        {
            unsafe
            {
                return NativeMethods.operator_copy_async(
                    this,
                    sourcePath,
                    targetPath,
                    &OnCopyCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Renames a file from source path to target path.
    /// </summary>
    /// <param name="sourcePath">Source path in the configured backend.</param>
    /// <param name="targetPath">Target path in the configured backend.</param>
    public void Rename(string sourcePath, string targetPath)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        var result = NativeMethods.operator_rename(this, sourcePath, targetPath);
        ThrowIfErrorAndRelease(result);
    }

    /// <summary>
    /// Renames a file from source path to target path asynchronously.
    /// </summary>
    /// <param name="sourcePath">Source path in the configured backend.</param>
    /// <param name="targetPath">Target path in the configured backend.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that completes when the native callback reports completion.</returns>
    public Task RenameAsync(
        string sourcePath,
        string targetPath,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        return SubmitAsyncOperation(SubmitRenameAsync, cancellationToken);

        OpenDALResult SubmitRenameAsync(long context)
        {
            unsafe
            {
                return NativeMethods.operator_rename_async(
                    this,
                    sourcePath,
                    targetPath,
                    &OnRenameCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Removes all entries under the specified path recursively.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    public void RemoveAll(string path)
    {
        var options = new DeleteOptions { Recursive = true };
        Delete(path, options);
    }

    /// <summary>
    /// Removes all entries under the specified path recursively asynchronously.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="cancellationToken">Cancellation token for the managed task.</param>
    /// <returns>A task that completes when the native callback reports completion.</returns>
    public Task RemoveAllAsync(string path, CancellationToken cancellationToken = default)
    {
        var options = new DeleteOptions { Recursive = true };
        return DeleteAsync(path, options, cancellationToken);
    }

    /// <summary>
    /// Creates a presigned read request asynchronously.
    /// </summary>
    public Task<PresignedRequest> PresignReadAsync(
        string path,
        TimeSpan expiration,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        var expireNanos = Utilities.ToNanoseconds(expiration, nameof(expiration));

        return SubmitAsyncOperation<PresignedRequest>(SubmitPresignReadAsync, cancellationToken);

        OpenDALResult SubmitPresignReadAsync(long context)
        {
            unsafe
            {
                return NativeMethods.operator_presign_read_async(
                    this,
                    path,
                    expireNanos,
                    &OnPresignReadCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Creates a presigned write request asynchronously.
    /// </summary>
    public Task<PresignedRequest> PresignWriteAsync(
        string path,
        TimeSpan expiration,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        var expireNanos = Utilities.ToNanoseconds(expiration, nameof(expiration));

        return SubmitAsyncOperation<PresignedRequest>(SubmitPresignWriteAsync, cancellationToken);

        OpenDALResult SubmitPresignWriteAsync(long context)
        {
            unsafe
            {
                return NativeMethods.operator_presign_write_async(
                    this,
                    path,
                    expireNanos,
                    &OnPresignWriteCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Creates a presigned stat request asynchronously.
    /// </summary>
    public Task<PresignedRequest> PresignStatAsync(
        string path,
        TimeSpan expiration,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        var expireNanos = Utilities.ToNanoseconds(expiration, nameof(expiration));

        return SubmitAsyncOperation<PresignedRequest>(SubmitPresignStatAsync, cancellationToken);

        OpenDALResult SubmitPresignStatAsync(long context)
        {
            unsafe
            {
                return NativeMethods.operator_presign_stat_async(
                    this,
                    path,
                    expireNanos,
                    &OnPresignStatCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Creates a presigned delete request asynchronously.
    /// </summary>
    public Task<PresignedRequest> PresignDeleteAsync(
        string path,
        TimeSpan expiration,
        CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);
        var expireNanos = Utilities.ToNanoseconds(expiration, nameof(expiration));

        return SubmitAsyncOperation<PresignedRequest>(SubmitPresignDeleteAsync, cancellationToken);

        OpenDALResult SubmitPresignDeleteAsync(long context)
        {
            unsafe
            {
                return NativeMethods.operator_presign_delete_async(
                    this,
                    path,
                    expireNanos,
                    &OnPresignDeleteCompleted,
                    context
                );
            }
        }
    }

    /// <summary>
    /// Opens a read stream for the specified path.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="options">Optional read options.</param>
    /// <returns>A readable stream over the given path.</returns>
    public OperatorInputStream OpenReadStream(
        string path,
        ReadOptions? options = null)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        using var nativeOptions = options?.BuildNativeOptionsHandle();
        var result = NativeMethods.operator_input_stream_create(
            this,
            path,
            GetOptionsHandle(nativeOptions)
        );

        var streamHandle = ToValueOrThrowAndRelease<IntPtr, OpenDALOperatorResult>(result);
        if (streamHandle == IntPtr.Zero)
        {
            throw new InvalidOperationException("OpenReadStream returned null stream pointer");
        }

        return new OperatorInputStream(streamHandle);
    }

    /// <summary>
    /// Opens a write stream for the specified path.
    /// </summary>
    /// <param name="path">Target path in the configured backend.</param>
    /// <param name="options">Optional write options.</param>
    /// <param name="bufferSize">Buffer size used by the managed write stream.</param>
    /// <returns>A writable stream over the given path.</returns>
    public OperatorOutputStream OpenWriteStream(
        string path,
        WriteOptions? options = null,
        int bufferSize = OperatorOutputStream.DefaultBufferSize)
    {
        ObjectDisposedException.ThrowIf(IsInvalid, this);

        using var nativeOptions = options?.BuildNativeOptionsHandle();
        var result = NativeMethods.operator_output_stream_create(
            this,
            path,
            GetOptionsHandle(nativeOptions)
        );

        var streamHandle = ToValueOrThrowAndRelease<IntPtr, OpenDALOperatorResult>(result);
        if (streamHandle == IntPtr.Zero)
        {
            throw new InvalidOperationException("OpenWriteStream returned null stream pointer");
        }

        return new OperatorOutputStream(streamHandle, bufferSize);
    }

    /// <summary>
    /// Releases the native operator handle.
    /// </summary>
    /// <returns><see langword="true"/> after the handle has been released.</returns>
    protected override bool ReleaseHandle()
    {
        NativeMethods.operator_free(handle);
        return true;
    }

    /// <summary>
    /// Applies a native layer result by creating a new operator from the returned handle.
    /// </summary>
    /// <param name="result">Native result that contains a new operator pointer.</param>
    /// <returns>A new operator instance.</returns>
    /// <exception cref="InvalidOperationException">Returned operator pointer is null.</exception>
    /// <exception cref="OpenDALException">Native layer application fails.</exception>
    internal Operator ApplyLayerResult(OpenDALOperatorResult result)
    {
        var newHandle = ToValueOrThrowAndRelease<IntPtr, OpenDALOperatorResult>(result);
        if (newHandle == IntPtr.Zero)
        {
            throw new InvalidOperationException("Layer application returned null operator pointer");
        }

        return new Operator(newHandle);
    }

    /// <summary>
    /// Gets the native options pointer from an optional native options handle.
    /// </summary>
    /// <param name="options">Native options handle or <see langword="null"/>.</param>
    /// <returns>Native options pointer, or <see cref="IntPtr.Zero"/> when options are not provided.</returns>
    private static IntPtr GetOptionsHandle(NativeOptionsHandle? options)
    {
        return options is null ? IntPtr.Zero : options.DangerousGetHandle();
    }

    /// <summary>
    /// Retrieves operator info from the native layer.
    /// </summary>
    /// <returns>Managed operator info value.</returns>
    /// <exception cref="OpenDALException">Native operator info retrieval fails.</exception>
    private OperatorInfo CreateOperatorInfo()
    {
        var result = NativeMethods.operator_info_get(this);

        return ToValueOrThrowAndRelease<OperatorInfo, OpenDALOperatorInfoResult>(result);
    }

    /// <summary>
    /// Builds constructor options for operator creation when key/value options are provided.
    /// </summary>
    /// <param name="options">Backend options dictionary.</param>
    /// <returns>Native options handle, or <see langword="null"/> when options are empty.</returns>
    private static NativeOptionsHandle? CreateConstructorOptionsHandle(IReadOnlyDictionary<string, string>? options)
    {
        if (options is null || options.Count == 0)
        {
            return null;
        }

        return NativeOptionsBuilder.BuildNativeOptionsHandle(
            options,
            NativeMethods.constructor_option_build,
            NativeMethods.constructor_option_free
        );
    }

    /// <summary>
    /// Converts a native result into a managed value, throwing on native error and always releasing native resources.
    /// </summary>
    /// <typeparam name="TOutput">Managed output type.</typeparam>
    /// <typeparam name="TResult">Native result type.</typeparam>
    /// <param name="result">Native result payload.</param>
    /// <returns>Managed value converted from <paramref name="result"/>.</returns>
    /// <exception cref="OpenDALException">Native operation returns an error.</exception>
    internal static TOutput ToValueOrThrowAndRelease<TOutput, TResult>(TResult result)
        where TResult : struct, INativeValueResult<TOutput>
    {
        try
        {
            var error = result.GetError();
            if (error.IsError)
            {
                throw new OpenDALException(error);
            }

            return result.ToValue();
        }
        finally
        {
            result.Release();
        }
    }

    /// <summary>
    /// Throws when a native result reports an error and always releases native resources.
    /// </summary>
    /// <typeparam name="TResult">Native result type.</typeparam>
    /// <param name="result">Native result payload.</param>
    /// <exception cref="OpenDALException">Native operation returns an error.</exception>
    internal static void ThrowIfErrorAndRelease<TResult>(TResult result)
        where TResult : struct, INativeResult
    {
        try
        {
            var error = result.GetError();
            if (error.IsError)
            {
                throw new OpenDALException(error);
            }
        }
        finally
        {
            result.Release();
        }
    }

    /// <summary>
    /// Submits a native async operation and binds it to a managed task completion source.
    /// </summary>
    /// <typeparam name="TOutput">Managed task result type.</typeparam>
    /// <typeparam name="TOptions">Managed options type.</typeparam>
    /// <param name="options">Optional managed options for this operation.</param>
    /// <param name="submit">Submission delegate that invokes the native async API.</param>
    /// <param name="cancellationToken">Cancellation token for managed task observation.</param>
    /// <returns>A task completed by the corresponding native callback.</returns>
    /// <exception cref="OperationCanceledException"><paramref name="cancellationToken"/> is already canceled.</exception>
    /// <exception cref="OpenDALException">Native submission returns an immediate error.</exception>
    internal static Task<TOutput> SubmitAsyncOperation<TOutput, TOptions>(
        TOptions? options,
        Func<long, IntPtr, OpenDALResult> submit,
        CancellationToken cancellationToken)
        where TOptions : class, IOptions
    {
        cancellationToken.ThrowIfCancellationRequested();
        var context = AsyncStateRegistry.Register<TOutput>(out var asyncState);
        try
        {
            using var nativeOptionsHandle = options?.BuildNativeOptionsHandle();
            var submitResult = submit(context, GetOptionsHandle(nativeOptionsHandle));
            ThrowIfErrorAndRelease(submitResult);
            asyncState.BindCancellation(cancellationToken);
            return asyncState.Completion.Task;
        }
        catch
        {
            AsyncStateRegistry.Unregister(context);
            throw;
        }
    }

    /// <summary>
    /// Submits a native async operation and binds it to a managed task completion source.
    /// </summary>
    /// <typeparam name="TOptions">Managed options type.</typeparam>
    /// <param name="options">Optional managed options for this operation.</param>
    /// <param name="submit">Submission delegate that invokes the native async API.</param>
    /// <param name="cancellationToken">Cancellation token for managed task observation.</param>
    /// <returns>A task completed by the corresponding native callback.</returns>
    /// <exception cref="OperationCanceledException"><paramref name="cancellationToken"/> is already canceled.</exception>
    /// <exception cref="OpenDALException">Native submission returns an immediate error.</exception>
    internal static Task SubmitAsyncOperation<TOptions>(
        TOptions? options,
        Func<long, IntPtr, OpenDALResult> submit,
        CancellationToken cancellationToken)
        where TOptions : class, IOptions
    {
        return SubmitAsyncOperation<bool, TOptions>(options, submit, cancellationToken);
    }

    /// <summary>
    /// Submits a native async operation without options and binds it to a managed task completion source.
    /// </summary>
    /// <typeparam name="TOutput">Managed task result type.</typeparam>
    /// <param name="submit">Submission delegate that invokes the native async API.</param>
    /// <param name="cancellationToken">Cancellation token for managed task observation.</param>
    /// <returns>A task completed by the corresponding native callback.</returns>
    internal static Task<TOutput> SubmitAsyncOperation<TOutput>(
        Func<long, OpenDALResult> submit,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var context = AsyncStateRegistry.Register<TOutput>(out var asyncState);
        try
        {
            var submitResult = submit(context);
            ThrowIfErrorAndRelease(submitResult);
            asyncState.BindCancellation(cancellationToken);
            return asyncState.Completion.Task;
        }
        catch
        {
            AsyncStateRegistry.Unregister(context);
            throw;
        }
    }

    /// <summary>
    /// Submits a native async operation without options and binds it to a managed task completion source.
    /// </summary>
    /// <param name="submit">Submission delegate that invokes the native async API.</param>
    /// <param name="cancellationToken">Cancellation token for managed task observation.</param>
    /// <returns>A task completed by the corresponding native callback.</returns>
    /// <exception cref="OperationCanceledException"><paramref name="cancellationToken"/> is already canceled.</exception>
    /// <exception cref="OpenDALException">Native submission returns an immediate error.</exception>
    internal static Task SubmitAsyncOperation(
        Func<long, OpenDALResult> submit,
        CancellationToken cancellationToken)
    {
        return SubmitAsyncOperation<bool>(submit, cancellationToken);
    }

    /// <summary>
    /// Completes a value-producing async state from a native callback result.
    /// </summary>
    /// <typeparam name="TOutput">Managed output type.</typeparam>
    /// <typeparam name="TResult">Native result type.</typeparam>
    /// <param name="context">Native callback context id.</param>
    /// <param name="result">Native callback result payload.</param>
    private static void CompleteAsyncState<TOutput, TResult>(long context, TResult result)
        where TResult : struct, INativeValueResult<TOutput>
    {
        if (!AsyncStateRegistry.TryTake<AsyncState<TOutput>>(context, out var state))
        {
            return;
        }

        try
        {
            state.CancellationRegistration.Dispose();

            if (result.GetError().IsError)
            {
                state.Completion.TrySetException(new OpenDALException(result.GetError()));
                return;
            }

            state.Completion.TrySetResult(result.ToValue());
        }
        catch (Exception ex)
        {
            state.Completion.TrySetException(ex);
        }
    }

    /// <summary>
    /// Completes a non-value async state from a native callback result.
    /// </summary>
    /// <typeparam name="TResult">Native result type.</typeparam>
    /// <param name="context">Native callback context id.</param>
    /// <param name="result">Native callback result payload.</param>
    private static void CompleteAsyncState<TResult>(long context, TResult result)
        where TResult : struct, INativeResult
    {
        if (!AsyncStateRegistry.TryTake<AsyncState<bool>>(context, out var state))
        {
            return;
        }

        try
        {
            state.CancellationRegistration.Dispose();

            var error = result.GetError();
            if (error.IsError)
            {
                state.Completion.TrySetException(new OpenDALException(error));
                return;
            }

            state.Completion.TrySetResult(true);
        }
        catch (Exception ex)
        {
            state.Completion.TrySetException(ex);
        }
    }

    /// <summary>
    /// Finalizes a value-producing native callback by completing managed state and releasing native resources.
    /// </summary>
    /// <typeparam name="TOutput">Managed output type.</typeparam>
    /// <typeparam name="TResult">Native result type.</typeparam>
    /// <param name="context">Native callback context id.</param>
    /// <param name="result">Native callback result payload.</param>
    internal static void CompleteAsyncCallback<TOutput, TResult>(long context, TResult result)
        where TResult : struct, INativeValueResult<TOutput>
    {
        try
        {
            CompleteAsyncState<TOutput, TResult>(context, result);
        }
        finally
        {
            result.Release();
        }
    }

    /// <summary>
    /// Finalizes a non-value native callback by completing managed state and releasing native resources.
    /// </summary>
    /// <typeparam name="TResult">Native result type.</typeparam>
    /// <param name="context">Native callback context id.</param>
    /// <param name="result">Native callback result payload.</param>
    internal static void CompleteAsyncCallback<TResult>(long context, TResult result)
        where TResult : struct, INativeResult
    {
        try
        {
            CompleteAsyncState(context, result);
        }
        finally
        {
            result.Release();
        }
    }

    #region Async Callbacks

    /// <summary>
    /// Native callback invoked when an asynchronous write operation finishes.
    /// </summary>
    /// <param name="context">Opaque async state context previously registered by <see cref="AsyncStateRegistry"/>.</param>
    /// <param name="result">Write completion result returned by the native layer.</param>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnWriteCompleted(long context, OpenDALMetadataResult result)
    {
        CompleteAsyncCallback<Metadata, OpenDALMetadataResult>(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous stat operation finishes.
    /// </summary>
    /// <param name="context">Opaque async state context previously registered by <see cref="AsyncStateRegistry"/>.</param>
    /// <param name="result">Stat completion result returned by the native layer.</param>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnStatCompleted(long context, OpenDALMetadataResult result)
    {
        CompleteAsyncCallback<Metadata, OpenDALMetadataResult>(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous exists operation finishes.
    /// </summary>
    /// <param name="context">Opaque async state context previously registered by <see cref="AsyncStateRegistry"/>.</param>
    /// <param name="result">Exists completion result returned by the native layer.</param>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnExistsCompleted(long context, OpenDALBoolResult result)
    {
        CompleteAsyncCallback<bool, OpenDALBoolResult>(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous check operation finishes.
    /// </summary>
    /// <param name="context">Opaque async state context previously registered by <see cref="AsyncStateRegistry"/>.</param>
    /// <param name="result">Check completion result returned by the native layer.</param>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnCheckCompleted(long context, OpenDALResult result)
    {
        CompleteAsyncCallback(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous list operation finishes.
    /// </summary>
    /// <param name="context">Opaque async state context previously registered by <see cref="AsyncStateRegistry"/>.</param>
    /// <param name="result">List completion result returned by the native layer.</param>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnListCompleted(long context, OpenDALEntryListResult result)
    {
        CompleteAsyncCallback<IReadOnlyList<Entry>, OpenDALEntryListResult>(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous delete operation finishes.
    /// </summary>
    /// <param name="context">Opaque async state context previously registered by <see cref="AsyncStateRegistry"/>.</param>
    /// <param name="result">Delete completion result returned by the native layer.</param>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnDeleteCompleted(long context, OpenDALResult result)
    {
        CompleteAsyncCallback(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous create-dir operation finishes.
    /// </summary>
    /// <param name="context">Opaque async state context previously registered by <see cref="AsyncStateRegistry"/>.</param>
    /// <param name="result">Create-dir completion result returned by the native layer.</param>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnCreateDirCompleted(long context, OpenDALResult result)
    {
        CompleteAsyncCallback(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous copy operation finishes.
    /// </summary>
    /// <param name="context">Opaque async state context previously registered by <see cref="AsyncStateRegistry"/>.</param>
    /// <param name="result">Copy completion result returned by the native layer.</param>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnCopyCompleted(long context, OpenDALMetadataResult result)
    {
        CompleteAsyncCallback<Metadata, OpenDALMetadataResult>(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous rename operation finishes.
    /// </summary>
    /// <param name="context">Opaque async state context previously registered by <see cref="AsyncStateRegistry"/>.</param>
    /// <param name="result">Rename completion result returned by the native layer.</param>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnRenameCompleted(long context, OpenDALResult result)
    {
        CompleteAsyncCallback(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous presign-read operation finishes.
    /// </summary>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnPresignReadCompleted(long context, OpenDALPresignedRequestResult result)
    {
        CompleteAsyncCallback<PresignedRequest, OpenDALPresignedRequestResult>(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous presign-write operation finishes.
    /// </summary>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnPresignWriteCompleted(long context, OpenDALPresignedRequestResult result)
    {
        CompleteAsyncCallback<PresignedRequest, OpenDALPresignedRequestResult>(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous presign-stat operation finishes.
    /// </summary>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnPresignStatCompleted(long context, OpenDALPresignedRequestResult result)
    {
        CompleteAsyncCallback<PresignedRequest, OpenDALPresignedRequestResult>(context, result);
    }

    /// <summary>
    /// Native callback invoked when an asynchronous presign-delete operation finishes.
    /// </summary>
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnPresignDeleteCompleted(long context, OpenDALPresignedRequestResult result)
    {
        CompleteAsyncCallback<PresignedRequest, OpenDALPresignedRequestResult>(context, result);
    }

    #endregion

}
