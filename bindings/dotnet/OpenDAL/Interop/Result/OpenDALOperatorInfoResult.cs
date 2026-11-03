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
/// Result wrapper for operations that return an operator info payload.
/// </summary>
internal struct OpenDALOperatorInfoResult : INativeValueResult<OperatorInfo>
{
    public IntPtr Ptr;

    public OpenDALError Error;

    public readonly void Release()
    {
        NativeMethods.opendal_operator_info_result_release(this);
    }

    public readonly OpenDALError GetError()
    {
        return Error;
    }

    public readonly unsafe OperatorInfo ToValue()
    {
        if (Ptr == IntPtr.Zero)
        {
            throw new InvalidOperationException("operator_info_get returned null pointer");
        }

        var payload = Unsafe.Read<OpenDALOperatorInfo>((void*)Ptr);
        return new OperatorInfo(
            Utilities.ReadUtf8(payload.Scheme),
            Utilities.ReadUtf8(payload.Root),
            Utilities.ReadUtf8(payload.Name),
            new Capability(payload.Capability)
        );
    }
}
