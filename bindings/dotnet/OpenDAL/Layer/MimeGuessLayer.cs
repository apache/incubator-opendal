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

using OpenDAL.Layer.Abstractions;

namespace OpenDAL.Layer;

/// <summary>
/// Layer that derives <c>Content-Type</c> from the file extension in the operation path.
/// </summary>
/// <remarks>
/// An existing content type always wins: the layer fills in a value only when the caller
/// did not set one on write, and only when the service did not report one on stat.
/// Paths whose extension cannot be resolved are left untouched.
/// </remarks>
public sealed class MimeGuessLayer : ILayer
{
    /// <summary>
    /// Applies MIME-guess behavior to the specified operator.
    /// </summary>
    /// <param name="op">Operator to layer.</param>
    /// <returns>The layered operator instance.</returns>
    public Operator Apply(Operator op)
    {
        ArgumentNullException.ThrowIfNull(op);
        ObjectDisposedException.ThrowIf(op.IsInvalid, op);

        var result = NativeMethods.operator_layer_mime_guess(op);
        return op.ApplyLayerResult(result);
    }
}
