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
using System.Text;
using OpenDAL.Interop.NativeObject;
using OpenDAL.Options.Abstractions;

namespace OpenDAL.Options;

/// <summary>
/// Writes options into one buffer that native code borrows for a single
/// call: the native struct, then every string, string-pair array, and
/// nested metadata it points to.
/// </summary>
/// <remarks>
/// Pointers into the buffer are written as they go, so the buffer cannot
/// grow. Once a write does not fit, the writer stops writing but keeps
/// counting, so <see cref="RequiredCapacity"/> tells the caller how large a
/// buffer to write the same options into again.
/// </remarks>
internal unsafe ref struct NativeOptionsWriter
{
    private const int Alignment = 8;

    private readonly byte* start;
    private readonly int capacity;
    private int required;

    /// <summary>
    /// Creates a writer over <paramref name="length"/> bytes at
    /// <paramref name="buffer"/>, which may be null to only count.
    /// </summary>
    internal NativeOptionsWriter(byte* buffer, int length)
    {
        // Align the start, so aligning offsets also aligns addresses.
        start = (byte*)(((nuint)buffer + Alignment - 1) & ~(nuint)(Alignment - 1));
        capacity = buffer is null ? 0 : Math.Max(0, length - (int)(start - buffer));
        required = 0;
        Overflowed = false;
    }

    /// <summary>
    /// Whether a write did not fit, which leaves the struct incomplete.
    /// </summary>
    public bool Overflowed { get; private set; }

    /// <summary>
    /// Bytes of a buffer that fits everything written so far, including the
    /// padding needed to align its start.
    /// </summary>
    public int RequiredCapacity => required + Alignment - 1;

    /// <summary>
    /// Reserves an aligned, uninitialized <typeparamref name="T"/>, or returns
    /// null when it does not fit.
    /// </summary>
    public T* Reserve<T>() where T : unmanaged => (T*)Allocate(sizeof(T), Alignment);

    /// <summary>
    /// Writes an option string, where null and empty both mean unset.
    /// </summary>
    public IntPtr String(string? value) => string.IsNullOrEmpty(value) ? IntPtr.Zero : Utf8(value);

    /// <summary>
    /// Writes a null-terminated UTF-8 string, keeping an empty one.
    /// </summary>
    public IntPtr Utf8(string? value)
    {
        if (value is null)
        {
            return IntPtr.Zero;
        }

        var length = Encoding.UTF8.GetByteCount(value);
        var target = Allocate(checked(length + 1), 1);
        if (target is null)
        {
            return IntPtr.Zero;
        }

        Encoding.UTF8.GetBytes(value, new Span<byte>(target, length));
        target[length] = 0;
        return (IntPtr)target;
    }

    /// <summary>
    /// Writes string pairs as two parallel arrays of string pointers. A null
    /// value stays a null pointer, which native code reports by key.
    /// </summary>
    public (byte HasValue, IntPtr Keys, IntPtr Values, nuint Len) Pairs(IReadOnlyDictionary<string, string>? pairs)
    {
        if (pairs is null)
        {
            return default;
        }

        var count = pairs.Count;
        var keys = (IntPtr*)Allocate(checked(count * IntPtr.Size), Alignment);
        var values = (IntPtr*)Allocate(checked(count * IntPtr.Size), Alignment);
        var index = 0;
        foreach (var entry in pairs)
        {
            if (index == count)
            {
                throw Changed();
            }

            // Count the strings even when the arrays did not fit.
            var key = Utf8(entry.Key);
            var value = Utf8(entry.Value);
            if (keys is not null && values is not null)
            {
                keys[index] = key;
                values[index] = value;
            }

            index++;
        }

        if (index != count)
        {
            throw Changed();
        }

        return (1, (IntPtr)keys, (IntPtr)values, (nuint)count);
    }

    /// <summary>
    /// Writes metadata for native code to rebuild, or returns null for none.
    /// </summary>
    public IntPtr MetadataPointer(Metadata? metadata)
    {
        if (metadata is null)
        {
            return IntPtr.Zero;
        }

        var target = Reserve<OpenDALMetadata>();
        var native = OpenDALMetadata.From(metadata, ref this);
        if (target is null)
        {
            return IntPtr.Zero;
        }

        *target = native;
        return (IntPtr)target;
    }

    /// <summary>
    /// The exception thrown when options change between counting and
    /// writing, for example a dictionary modified by another thread.
    /// </summary>
    internal static InvalidOperationException Changed() =>
        new("Options changed while they were being passed to native code.");

    private byte* Allocate(int size, int alignment)
    {
        var offset = (required + alignment - 1) & ~(alignment - 1);
        required = checked(offset + size);
        if (Overflowed || required > capacity)
        {
            Overflowed = true;
            return null;
        }

        return start + offset;
    }
}

/// <summary>
/// The options buffer of one native call. Disposing it frees a buffer that
/// did not fit on the caller's stack.
/// </summary>
internal readonly unsafe struct NativeOptionsScope<TNative> : IDisposable where TNative : unmanaged
{
    private readonly void* heap;

    internal NativeOptionsScope(TNative* pointer, void* heap)
    {
        Pointer = pointer;
        this.heap = heap;
    }

    /// <summary>
    /// The options for native code, or null for the defaults.
    /// </summary>
    public TNative* Pointer { get; }

    public void Dispose()
    {
        if (heap is not null)
        {
            NativeMemory.Free(heap);
        }
    }
}

/// <summary>
/// Creates <see cref="NativeOptionsScope{TNative}"/> values.
/// </summary>
internal static unsafe class NativeOptionsScope
{
    /// <summary>
    /// Size of the stack buffer callers pass to <see cref="Create"/>. Options
    /// that need more go to native memory instead.
    /// </summary>
    public const int StackSize = 512;

    /// <summary>
    /// Validates <paramref name="options"/> and writes them for one native
    /// call: into <paramref name="stack"/> in a single pass when they fit,
    /// otherwise once more into native memory of the size the first pass
    /// counted.
    /// </summary>
    /// <remarks>
    /// <paramref name="stack"/> must be stack memory of the caller, which
    /// must dispose the scope before returning and must not use
    /// <see cref="NativeOptionsScope{TNative}.Pointer"/> after that.
    /// </remarks>
    public static NativeOptionsScope<TNative> Create<TNative>(IOptions<TNative>? options, Span<byte> stack)
        where TNative : unmanaged
    {
        if (options is null)
        {
            return default;
        }

        options.Validate();
        var writer = new NativeOptionsWriter(
            (byte*)Unsafe.AsPointer(ref MemoryMarshal.GetReference(stack)),
            stack.Length);
        var native = writer.Reserve<TNative>();
        var value = options.ToNative(ref writer);
        if (!writer.Overflowed)
        {
            *native = value;
            return new NativeOptionsScope<TNative>(native, null);
        }

        var size = writer.RequiredCapacity;
        var heap = NativeMemory.Alloc((nuint)size);
        try
        {
            writer = new NativeOptionsWriter((byte*)heap, size);
            native = writer.Reserve<TNative>();
            value = options.ToNative(ref writer);
            if (writer.Overflowed)
            {
                throw NativeOptionsWriter.Changed();
            }

            *native = value;
            return new NativeOptionsScope<TNative>(native, heap);
        }
        catch
        {
            NativeMemory.Free(heap);
            throw;
        }
    }
}
