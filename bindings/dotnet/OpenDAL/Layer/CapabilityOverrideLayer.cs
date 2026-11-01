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
/// Layer that overrides the full capability exposed by an operator.
/// </summary>
public sealed class CapabilityOverrideLayer : ILayer
{
    /// <summary>
    /// Creates a capability override layer.
    /// </summary>
    /// <param name="overrides">Comma-separated capability override entries.</param>
    public CapabilityOverrideLayer(string overrides)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(overrides);
        Overrides = overrides;
    }

    /// <summary>
    /// Gets capability override entries.
    /// </summary>
    public string Overrides { get; }

    /// <summary>
    /// Applies capability overrides to the specified operator.
    /// </summary>
    /// <param name="op">Operator to layer.</param>
    /// <returns>The layered operator instance.</returns>
    public Operator Apply(Operator op)
    {
        ArgumentNullException.ThrowIfNull(op);
        ObjectDisposedException.ThrowIf(op.IsInvalid, op);

        var result = NativeMethods.operator_layer_capability_override(op, Overrides);
        return op.ApplyLayerResult(result);
    }
}
