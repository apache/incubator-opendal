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

using System.Runtime.InteropServices;

namespace OpenDAL.Interop.NativeObject;

[StructLayout(LayoutKind.Sequential)]
/// <summary>
/// Read payload still owned by the native side.
/// </summary>
/// <remarks>
/// Holds the native buffer itself rather than a flattened copy, so consumers
/// copy or view chunks straight from native memory. Released through the
/// owning result's release call, not from here.
/// </remarks>
internal struct OpenDALReadBuffer
{
    /// <summary>
    /// Opaque handle to the native buffer, or zero when there is no payload.
    /// </summary>
    public IntPtr Handle;

    /// <summary>
    /// Total readable bytes across every chunk.
    /// </summary>
    public nuint Len;

    /// <summary>
    /// Copies payload bytes starting at <paramref name="sourceOffset"/> into the
    /// destination span.
    /// </summary>
    /// <param name="sourceOffset">Byte offset into the payload to copy from.</param>
    /// <param name="destination">Destination span receiving the bytes.</param>
    /// <returns>
    /// The number of bytes copied; 0 when the offset is at or past the end of the
    /// payload or when there is no payload.
    /// </returns>
    public readonly unsafe int CopyTo(nuint sourceOffset, Span<byte> destination)
    {
        if (Handle == IntPtr.Zero || destination.Length == 0)
        {
            return 0;
        }

        fixed (byte* dest = destination)
        {
            return checked((int)NativeMethods.read_buffer_copy_to(
                Handle, sourceOffset, dest, (nuint)destination.Length));
        }
    }
}
