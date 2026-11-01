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

using OpenDAL.Options.Abstractions;

namespace OpenDAL.Options;

/// <summary>
/// Options for delete operations.
/// </summary>
public sealed class DeleteOptions : IOptions
{
    /// <summary>
    /// The version of the file to delete.
    /// </summary>
    public string? Version { get; init; }

    /// <summary>
    /// Whether to delete the target recursively.
    /// </summary>
    /// <remarks>
    /// If `false`, behaves like the traditional single-object delete.
    /// If `true`, all entries under the path (or sharing the prefix for file-like paths) will be removed.
    /// </remarks>
    public bool Recursive { get; init; }

    /// <summary>
    /// Sets the condition that the delete operation will succeed only if the
    /// existing object's ETag matches the given value.
    /// </summary>
    /// <remarks>
    /// Check <see cref="Capability.DeleteWithIfMatch"/> before using this feature.
    /// If supported, the delete operation will only succeed when the existing
    /// object's ETag matches the given value.
    /// </remarks>
    public string? IfMatch { get; init; }

    public NativeOptionsHandle BuildNativeOptionsHandle()
    {
        var nativeOptions = new NativeOptionsBuilder()
            .AddString("version", Version)
            .AddBoolTrue("recursive", Recursive)
            .AddString("if_match", IfMatch)
            .Build();

        return NativeOptionsBuilder.BuildNativeOptionsHandle(
            nativeOptions,
            NativeMethods.delete_option_build,
            NativeMethods.delete_option_free
        );
    }
}