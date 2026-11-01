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

namespace OpenDAL.Interop.Buffers;

/// <summary>
/// Presents a fixed region of native memory as <see cref="Memory{T}"/>.
/// The region never moves, so pinning is a no-op; the caller guarantees the
/// memory outlives every handed-out view.
/// </summary>
internal sealed unsafe class NativeMemoryManager : MemoryManager<byte>
{
    private readonly byte* pointer;
    private readonly int length;

    public NativeMemoryManager(byte* pointer, int length)
    {
        this.pointer = pointer;
        this.length = length;
    }

    public override Span<byte> GetSpan()
    {
        return new Span<byte>(pointer, length);
    }

    public override MemoryHandle Pin(int elementIndex = 0)
    {
        return new MemoryHandle(pointer + elementIndex);
    }

    public override void Unpin()
    {
    }

    protected override void Dispose(bool disposing)
    {
    }
}
