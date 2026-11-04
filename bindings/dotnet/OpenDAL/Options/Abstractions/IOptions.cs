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

namespace OpenDAL.Options.Abstractions;

/// <summary>
/// Operation options that fill the native struct <typeparamref name="TNative"/>.
/// </summary>
internal interface IOptions<TNative> where TNative : unmanaged
{
    /// <summary>
    /// Validates the options before anything crosses the FFI. Options with
    /// nothing to validate keep this empty default.
    /// </summary>
    void Validate()
    {
    }

    /// <summary>
    /// Fills the native struct, writing everything it points to through
    /// <paramref name="writer"/>.
    /// </summary>
    /// <remarks>
    /// May run twice for one call, when the first buffer is too small, so it
    /// must have no side effects beyond the writer.
    /// </remarks>
    TNative ToNative(ref NativeOptionsWriter writer);
}
