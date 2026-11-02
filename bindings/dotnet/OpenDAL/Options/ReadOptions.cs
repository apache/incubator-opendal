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
/// Additional options for read operations.
/// </summary>
public sealed class ReadOptions : IOptions
{
    public long Offset { get; init; }

    public long? Length { get; init; }

    public string? Version { get; init; }

    public string? IfMatch { get; init; }

    public string? IfNoneMatch { get; init; }

    public DateTimeOffset? IfModifiedSince { get; init; }

    public DateTimeOffset? IfUnmodifiedSince { get; init; }

    public int Concurrent { get; init; } = 1;

    public long? Chunk { get; init; }

    public long? Gap { get; init; }

    public string? OverrideContentType { get; init; }

    public string? OverrideCacheControl { get; init; }

    public string? OverrideContentDisposition { get; init; }

    public NativeOptionsHandle BuildNativeOptionsHandle()
    {
        OptionValidators.RequireGreaterThanOrEqualZero(Offset, nameof(Offset));
        OptionValidators.RequireNullableGreaterThanOrEqualZero(Length, nameof(Length));
        OptionValidators.RequireGreaterThanZero(Concurrent, nameof(Concurrent));
        OptionValidators.RequireNullableGreaterThanZero(Chunk, nameof(Chunk));
        OptionValidators.RequireNullableGreaterThanZero(Gap, nameof(Gap));

        var nativeOptions = new NativeOptionsBuilder()
            .AddInt64IfNotDefault("offset", Offset, 0)
            .AddNullableInt64("length", Length)
            .AddString("version", Version)
            .AddString("if_match", IfMatch)
            .AddString("if_none_match", IfNoneMatch)
            .AddUnixTimeMilliseconds("if_modified_since", IfModifiedSince)
            .AddUnixTimeMilliseconds("if_unmodified_since", IfUnmodifiedSince)
            .AddInt32IfNotDefault("concurrent", Concurrent, 1)
            .AddNullableInt64("chunk", Chunk)
            .AddNullableInt64("gap", Gap)
            .AddString("override_content_type", OverrideContentType)
            .AddString("override_cache_control", OverrideCacheControl)
            .AddString("override_content_disposition", OverrideContentDisposition)
            .Build();

        return NativeOptionsBuilder.BuildNativeOptionsHandle(
            nativeOptions,
            NativeMethods.read_option_build,
            NativeMethods.read_option_free
        );
    }
}