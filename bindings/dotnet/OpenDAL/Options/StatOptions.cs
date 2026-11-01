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
/// Additional options for stat operations.
/// </summary>
public sealed class StatOptions : IOptions
{
    public string? Version { get; init; }

    public string? IfMatch { get; init; }

    public string? IfNoneMatch { get; init; }

    public DateTimeOffset? IfModifiedSince { get; init; }

    public DateTimeOffset? IfUnmodifiedSince { get; init; }

    public string? OverrideContentType { get; init; }

    public string? OverrideCacheControl { get; init; }

    public string? OverrideContentDisposition { get; init; }

    public NativeOptionsHandle BuildNativeOptionsHandle()
    {
        var nativeOptions = new NativeOptionsBuilder()
            .AddString("version", Version)
            .AddString("if_match", IfMatch)
            .AddString("if_none_match", IfNoneMatch)
            .AddUnixTimeMilliseconds("if_modified_since", IfModifiedSince)
            .AddUnixTimeMilliseconds("if_unmodified_since", IfUnmodifiedSince)
            .AddString("override_content_type", OverrideContentType)
            .AddString("override_cache_control", OverrideCacheControl)
            .AddString("override_content_disposition", OverrideContentDisposition)
            .Build();

        return NativeOptionsBuilder.BuildNativeOptionsHandle(
            nativeOptions,
            NativeMethods.stat_option_build,
            NativeMethods.stat_option_free
        );
    }
}