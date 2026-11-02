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

using OpenDAL.Interop.Result;

namespace OpenDAL.Options;

/// <summary>
/// Delegate that builds a native options payload from managed key/value arrays.
/// </summary>
/// <param name="keys">Option keys.</param>
/// <param name="values">Option values aligned by index with <paramref name="keys"/>.</param>
/// <param name="len">Number of key/value pairs.</param>
/// <returns>Native options build result.</returns>
internal delegate OpenDALOptionsResult NativeBuildOptionsDelegate(string[] keys, string[] values, nuint len);

/// <summary>
/// Builder that incrementally collects native key/value options.
/// </summary>
internal sealed class NativeOptionsBuilder
{
    private readonly Dictionary<string, string> options = new();

    /// <summary>
    /// Adds the option with value "true" when <paramref name="value"/> is true.
    /// </summary>
    public NativeOptionsBuilder AddBoolTrue(string key, bool value)
    {
        if (value)
        {
            options[key] = "true";
        }

        return this;
    }

    /// <summary>
    /// Adds the option when the string value is not null or empty.
    /// </summary>
    public NativeOptionsBuilder AddString(string key, string? value)
    {
        if (!string.IsNullOrEmpty(value))
        {
            options[key] = value;
        }

        return this;
    }

    /// <summary>
    /// Adds the option when the int value differs from the specified default.
    /// </summary>
    public NativeOptionsBuilder AddInt32IfNotDefault(string key, int value, int defaultValue)
    {
        if (value != defaultValue)
        {
            options[key] = value.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }

        return this;
    }

    /// <summary>
    /// Adds the option when the long value differs from the specified default.
    /// </summary>
    public NativeOptionsBuilder AddInt64IfNotDefault(string key, long value, long defaultValue)
    {
        if (value != defaultValue)
        {
            options[key] = value.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }

        return this;
    }

    /// <summary>
    /// Adds the option when the nullable long value is provided.
    /// </summary>
    public NativeOptionsBuilder AddNullableInt64(string key, long? value)
    {
        if (value is not null)
        {
            options[key] = value.Value.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }

        return this;
    }

    /// <summary>
    /// Adds the option as Unix time milliseconds when the timestamp is provided.
    /// </summary>
    public NativeOptionsBuilder AddUnixTimeMilliseconds(string key, DateTimeOffset? value)
    {
        if (value is not null)
        {
            options[key] = value.Value.ToUnixTimeMilliseconds().ToString(System.Globalization.CultureInfo.InvariantCulture);
        }

        return this;
    }

    /// <summary>
    /// Adds prefixed entries from the provided dictionary.
    /// </summary>
    public NativeOptionsBuilder AddPrefixedEntries(string prefix, IReadOnlyDictionary<string, string>? values)
    {
        if (values is null)
        {
            return this;
        }

        foreach (var entry in values)
        {
            options[$"{prefix}{entry.Key}"] = entry.Value;
        }

        return this;
    }

    /// <summary>
    /// Builds the final native options dictionary.
    /// </summary>
    public IReadOnlyDictionary<string, string> Build()
    {
        return options;
    }

    /// <summary>
    /// Builds a native options handle from managed key/value options.
    /// </summary>
    /// <param name="options">Managed options dictionary.</param>
    /// <param name="build">Native build function used to allocate and populate options payload.</param>
    /// <param name="release">Native release function used by the resulting handle.</param>
    /// <returns>A safe handle that owns the native options payload.</returns>
    /// <exception cref="ArgumentNullException"><paramref name="options"/> is null.</exception>
    /// <exception cref="OpenDALException">Native options build fails.</exception>
    public static NativeOptionsHandle BuildNativeOptionsHandle(
        IReadOnlyDictionary<string, string> options,
        NativeBuildOptionsDelegate build,
        Action<IntPtr> release)
    {
        ArgumentNullException.ThrowIfNull(options);

        var keys = new string[options.Count];
        var values = new string[options.Count];
        var index = 0;

        foreach (var option in options)
        {
            keys[index] = option.Key;
            values[index] = option.Value;
            index++;
        }

        var result = build(keys, values, (nuint)options.Count);
        var handle = Operator.ToValueOrThrowAndRelease<IntPtr, OpenDALOptionsResult>(result);
        return new NativeOptionsHandle(handle, release);
    }
}