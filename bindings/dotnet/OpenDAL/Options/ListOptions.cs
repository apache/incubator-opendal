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
/// Additional options for list operations.
/// </summary>
public sealed class ListOptions : IOptions
{
    public bool Recursive { get; init; }

    public long? Limit { get; init; }

    public string? StartAfter { get; init; }

    public bool Versions { get; init; }

    public bool Deleted { get; init; }

    public NativeOptionsHandle BuildNativeOptionsHandle()
    {
        OptionValidators.RequireNullableGreaterThanZero(Limit, nameof(Limit));

        var nativeOptions = new NativeOptionsBuilder()
            .AddBoolTrue("recursive", Recursive)
            .AddNullableInt64("limit", Limit)
            .AddString("start_after", StartAfter)
            .AddBoolTrue("versions", Versions)
            .AddBoolTrue("deleted", Deleted)
            .Build();

        return NativeOptionsBuilder.BuildNativeOptionsHandle(
            nativeOptions,
            NativeMethods.list_option_build,
            NativeMethods.list_option_free
        );
    }
}