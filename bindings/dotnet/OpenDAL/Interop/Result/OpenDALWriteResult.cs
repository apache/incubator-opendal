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
using OpenDAL.Interop.NativeObject;
using OpenDAL.Interop.Result.Abstractions;

namespace OpenDAL.Interop.Result;

[StructLayout(LayoutKind.Sequential)]
/// <summary>
/// Result wrapper for operations returning a freshly allocated native write buffer.
/// </summary>
/// <remarks>
/// On success the caller takes over the buffer handle, so
/// <see cref="Release"/> only frees the error message; the handle itself is
/// released later through <c>write_buffer_free</c>.
/// </remarks>
internal struct OpenDALWriteResult : INativeValueResult<OpenDALWriteBuffer>
{
    /// <summary>
    /// Allocated buffer payload on success.
    /// </summary>
    public OpenDALWriteBuffer Buffer;

    /// <summary>
    /// Error details for the operation.
    /// </summary>
    public OpenDALError Error;

    public readonly void Release()
    {
        NativeMethods.opendal_error_release(Error);
    }

    public readonly OpenDALError GetError()
    {
        return Error;
    }

    public readonly OpenDALWriteBuffer ToValue()
    {
        return Buffer;
    }
}
