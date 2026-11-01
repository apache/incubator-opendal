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
/// Writable native segment returned by <c>write_buffer_create</c> or
/// <c>write_buffer_add_segment</c>.
/// </summary>
/// <remarks>
/// <see cref="Data"/> points at <see cref="Capacity"/> writable bytes owned by
/// <see cref="Handle"/>. The handle must be released exactly once through
/// <c>write_buffer_free</c>.
/// </remarks>
internal struct OpenDALWriteBuffer
{
    /// <summary>
    /// Opaque handle owning every segment of the buffer.
    /// </summary>
    public IntPtr Handle;

    /// <summary>
    /// Raw pointer to the newest segment, valid until the handle is freed.
    /// </summary>
    public IntPtr Data;

    /// <summary>
    /// Writable bytes behind <see cref="Data"/>.
    /// </summary>
    public nuint Capacity;
}
