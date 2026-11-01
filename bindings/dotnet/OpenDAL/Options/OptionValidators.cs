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

namespace OpenDAL.Options;

/// <summary>
/// Common validation helpers for numeric option values.
/// </summary>
internal static class OptionValidators
{
    /// <summary>
    /// Ensures the value is greater than or equal to zero.
    /// </summary>
    public static void RequireGreaterThanOrEqualZero(long value, string paramName)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, $"{paramName} must be >= 0.");
        }
    }

    /// <summary>
    /// Ensures the nullable value is greater than or equal to zero when provided.
    /// </summary>
    public static void RequireNullableGreaterThanOrEqualZero(long? value, string paramName)
    {
        if (value is < 0)
        {
            throw new ArgumentOutOfRangeException(paramName, $"{paramName} must be >= 0.");
        }
    }

    /// <summary>
    /// Ensures the value is strictly greater than zero.
    /// </summary>
    public static void RequireGreaterThanZero(int value, string paramName)
    {
        if (value <= 0)
        {
            throw new ArgumentOutOfRangeException(paramName, $"{paramName} must be > 0.");
        }
    }

    /// <summary>
    /// Ensures the nullable value is strictly greater than zero when provided.
    /// </summary>
    public static void RequireNullableGreaterThanZero(long? value, string paramName)
    {
        if (value is <= 0)
        {
            throw new ArgumentOutOfRangeException(paramName, $"{paramName} must be > 0 when provided.");
        }
    }
}