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

namespace OpenDAL;

/// <summary>
/// Exception thrown when an OpenDAL native call returns an error.
/// </summary>
public class OpenDALException : Exception
{
    /// <summary>
    /// Gets the OpenDAL error code associated with this exception.
    /// Unknown native code values are normalized to <see cref="ErrorCode.Unexpected"/>.
    /// </summary>
    public ErrorCode Code { get; }

    /// <summary>
    /// Initializes a new exception from a native OpenDAL error payload.
    /// </summary>
    /// <param name="error">Error payload returned by the native binding.</param>
    /// <remarks>
    /// Native message text is decoded from UTF-8 and used as the exception message.
    /// </remarks>
    public OpenDALException(OpenDALError error) : base(Utilities.ReadUtf8(error.Message))
    {
        if (!TryParse(error.Code, out var parsed))
        {
            parsed = ErrorCode.Unexpected;
        }

        Code = parsed;
    }

    private static bool TryParse(int value, out ErrorCode result)
    {
        if ((uint)value <= (uint)ErrorCode.Conflict)
        {
            result = (ErrorCode)value;
            return true;
        }

        result = default;
        return false;
    }
}
