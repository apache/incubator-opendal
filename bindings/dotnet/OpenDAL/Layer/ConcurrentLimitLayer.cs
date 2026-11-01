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
/// Layer that limits concurrent operations on an operator.
/// </summary>
public sealed class ConcurrentLimitLayer : ILayer
{
    /// <summary>
    /// Gets maximum concurrent permits.
    /// </summary>
    public nuint Permits { get; }

    /// <summary>
    /// Gets maximum concurrent HTTP requests, or null when the HTTP limit is unset.
    /// </summary>
    /// <remarks>
    /// This limit is independent of <see cref="Permits"/>: a single operation may issue
    /// several HTTP requests, so limiting operations does not bound HTTP concurrency.
    /// </remarks>
    public nuint? HttpPermits { get; }

    /// <summary>
    /// Creates a concurrent-limit layer.
    /// </summary>
    /// <param name="permits">Maximum number of concurrent permits. Must be greater than zero.</param>
    /// <param name="httpPermits">
    /// Maximum number of concurrent HTTP requests, or null to leave the HTTP limit unset.
    /// Must be greater than zero when specified.
    /// </param>
    public ConcurrentLimitLayer(nuint permits, nuint? httpPermits = null)
    {
        if (permits == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(permits), "Permits must be greater than zero.");
        }

        if (httpPermits == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(httpPermits), "HttpPermits must be greater than zero when specified.");
        }

        Permits = permits;
        HttpPermits = httpPermits;
    }

    /// <summary>
    /// Applies concurrent-limit behavior to the specified operator.
    /// </summary>
    /// <param name="op">Operator to layer.</param>
    /// <returns>The layered operator instance.</returns>
    public Operator Apply(Operator op)
    {
        ArgumentNullException.ThrowIfNull(op);
        ObjectDisposedException.ThrowIf(op.IsInvalid, op);

        var result = NativeMethods.operator_layer_concurrent_limit(
            op,
            Permits,
            HttpPermits ?? 0,
            HttpPermits.HasValue);
        return op.ApplyLayerResult(result);
    }
}
