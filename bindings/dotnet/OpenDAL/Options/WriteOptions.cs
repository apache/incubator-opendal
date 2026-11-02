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
/// Additional options for write operations.
/// </summary>
public sealed class WriteOptions : IOptions
{
    public bool Append { get; init; }

    public string? CacheControl { get; init; }

    public string? ContentType { get; init; }

    public string? ContentDisposition { get; init; }

    public string? ContentEncoding { get; init; }

    public string? IfMatch { get; init; }

    public string? IfNoneMatch { get; init; }

    public bool IfNotExists { get; init; }

    public int Concurrent { get; init; } = 1;

    public long? Chunk { get; init; }

    public IReadOnlyDictionary<string, string>? UserMetadata { get; init; }

    public NativeOptionsHandle BuildNativeOptionsHandle()
    {
        OptionValidators.RequireGreaterThanZero(Concurrent, nameof(Concurrent));
        OptionValidators.RequireNullableGreaterThanZero(Chunk, nameof(Chunk));

        var nativeOptions = new NativeOptionsBuilder()
            .AddBoolTrue("append", Append)
            .AddString("cache_control", CacheControl)
            .AddString("content_type", ContentType)
            .AddString("content_disposition", ContentDisposition)
            .AddString("content_encoding", ContentEncoding)
            .AddString("if_match", IfMatch)
            .AddString("if_none_match", IfNoneMatch)
            .AddBoolTrue("if_not_exists", IfNotExists)
            .AddInt32IfNotDefault("concurrent", Concurrent, 1)
            .AddNullableInt64("chunk", Chunk)
            .AddPrefixedEntries("user_metadata.", UserMetadata)
            .Build();

        return NativeOptionsBuilder.BuildNativeOptionsHandle(
            nativeOptions,
            NativeMethods.write_option_build,
            NativeMethods.write_option_free
        );
    }
}