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
/// Layer that rate-limits the byte flow of read and write operations.
/// </summary>
/// <remarks>
/// Throttling applies to the reader and writer byte streams only. It does not limit
/// the number of requests, so it composes with <see cref="ConcurrentLimitLayer"/>
/// rather than replacing it.
/// </remarks>
public sealed class ThrottleLayer : ILayer
{
    /// <summary>
    /// Gets the maximum number of bytes allowed through per second.
    /// </summary>
    public uint Bandwidth { get; }

    /// <summary>
    /// Gets the maximum number of bytes allowed through at once.
    /// </summary>
    public uint Burst { get; }

    /// <summary>
    /// Creates a throttle layer.
    /// </summary>
    /// <param name="bandwidth">Maximum bytes per second. Must be greater than zero.</param>
    /// <param name="burst">
    /// Maximum bytes allowed through at once. Must be greater than zero, and must exceed the
    /// largest possible operation size: an operation larger than the burst size can never
    /// acquire enough quota and will block indefinitely.
    /// </param>
    public ThrottleLayer(uint bandwidth, uint burst)
    {
        if (bandwidth == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(bandwidth), "Bandwidth must be greater than zero.");
        }

        if (burst == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(burst), "Burst must be greater than zero.");
        }

        Bandwidth = bandwidth;
        Burst = burst;
    }

    /// <summary>
    /// Applies throttle behavior to the specified operator.
    /// </summary>
    /// <param name="op">Operator to layer.</param>
    /// <returns>The layered operator instance.</returns>
    public Operator Apply(Operator op)
    {
        ArgumentNullException.ThrowIfNull(op);
        ObjectDisposedException.ThrowIf(op.IsInvalid, op);

        var result = NativeMethods.operator_layer_throttle(op, Bandwidth, Burst);
        return op.ApplyLayerResult(result);
    }
}
