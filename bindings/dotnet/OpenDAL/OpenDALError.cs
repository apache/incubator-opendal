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

using System.Runtime.InteropServices;

namespace OpenDAL;

[StructLayout(LayoutKind.Sequential)]
/// <summary>
/// Error payload returned by native OpenDAL binding calls.
/// </summary>
public struct OpenDALError
{
    /// <summary>
    /// Non-zero when this instance represents an error.
    /// </summary>
    public byte HasError;

    /// <summary>
    /// Numeric error code defined by <see cref="ErrorCode"/>.
    /// </summary>
    public int Code;

    /// <summary>
    /// Pointer to a UTF-8 error message allocated by native code.
    /// </summary>
    public IntPtr Message;

    /// <summary>
    /// Gets whether this payload represents an error.
    /// </summary>
    public readonly bool IsError => HasError != 0;
}

/// <summary>
/// Represents all error kinds that may be returned by OpenDAL.
/// <para>
/// For details about each error code, see:
/// https://docs.rs/opendal/latest/opendal/enum.ErrorKind.html
/// </para>
/// </summary>
public enum ErrorCode
{
    /// <summary>
    /// OpenDAL don't know what happened here, and no actions other than just
    /// returning it back. For example, s3 returns an internal service error.
    /// </summary>
    Unexpected = 0,

    /// <summary>
    /// Underlying service doesn't support this operation.
    /// </summary>
    Unsupported = 1,

    /// <summary>
    /// The config for backend is invalid.
    /// </summary>
    ConfigInvalid = 2,

    /// <summary>
    /// The given path is not found.
    /// </summary>
    NotFound = 3,

    /// <summary>
    /// The given path doesn't have enough permission for this operation.
    /// </summary>
    PermissionDenied = 4,

    /// <summary>
    /// The given path is a directory.
    /// </summary>
    IsADirectory = 5,

    /// <summary>
    /// The given path is not a directory.
    /// </summary>
    NotADirectory = 6,

    /// <summary>
    /// The given path already exists thus we failed to the specified operation on it.
    /// </summary>
    AlreadyExists = 7,

    /// <summary>
    /// Requests that sent to this path is over the limit, please slow down.
    /// </summary>
    RateLimited = 8,

    /// <summary>
    /// The given file paths are same.
    /// </summary>
    IsSameFile = 9,

    /// <summary>
    /// The condition of this operation is not match.
    ///
    /// The `condition` itself is context based.
    ///
    /// For example, in S3, the `condition` can be:
    /// 1. writing a file with If-Match header but the file's ETag is not match (will get a 412 Precondition Failed).
    /// 2. reading a file with If-None-Match header but the file's ETag is match (will get a 304 Not Modified).
    ///
    /// As OpenDAL cannot handle the `condition not match` error, it will always return this error to users.
    /// So users could handle this error by themselves.
    /// </summary>
    ConditionNotMatch = 10,

    /// <summary>
    /// The range of the content is not satisfied.
    ///
    /// OpenDAL returns this error to indicate that the range of the read request is not satisfied.
    /// </summary>
    RangeNotSatisfied = 11,

    /// <summary>
    /// The operation conflicts with the current or transitional state of the resource.
    /// </summary>
    Conflict = 12,
}
