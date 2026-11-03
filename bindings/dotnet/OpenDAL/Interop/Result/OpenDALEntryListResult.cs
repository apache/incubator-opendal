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
using OpenDAL.Interop.NativeObject;
using OpenDAL.Interop.Result.Abstractions;

namespace OpenDAL.Interop.Result;

[StructLayout(LayoutKind.Sequential)]
/// <summary>
/// Result wrapper for operations that return an entry list payload.
/// </summary>
internal struct OpenDALEntryListResult : INativeValueResult<IReadOnlyList<Entry>>
{
    public IntPtr Ptr;

    public OpenDALError Error;

    public readonly OpenDALError GetError()
    {
        return Error;
    }

    /// <summary>
    /// Reads the native entry list and converts it into managed entries.
    /// </summary>
    /// <exception cref="InvalidOperationException">Thrown when the native list size exceeds <see cref="int.MaxValue"/>.</exception>
    public readonly unsafe IReadOnlyList<Entry> ToValue()
    {
        if (Ptr == IntPtr.Zero)
        {
            return Array.Empty<Entry>();
        }

        var payload = Unsafe.Read<OpenDALEntryList>((void*)Ptr);
        if (payload.Len > int.MaxValue)
        {
            throw new InvalidOperationException("Entry list too large");
        }

        var count = (int)payload.Len;
        var results = new List<Entry>(count);
        if (payload.Entries == IntPtr.Zero)
        {
            return results;
        }

        var entries = new ReadOnlySpan<OpenDALEntry>((void*)payload.Entries, count);
        for (var index = 0; index < count; index++)
        {
            ref readonly var entryPayload = ref entries[index];
            var path = Utilities.ReadUtf8(entryPayload.Path);
            results.Add(new Entry(path, entryPayload.Metadata.ToMetadata()));
        }

        return results;
    }

    public readonly void Release()
    {
        NativeMethods.opendal_entry_list_result_release(this);
    }
}
