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
/// Timeout layer configuration for operation and I/O deadlines.
/// </summary>
public sealed class TimeoutLayer : ILayer
{
    /// <summary>
    /// Gets total operation timeout.
    /// </summary>
    public TimeSpan Timeout { get; init; } = TimeSpan.FromSeconds(60);

    /// <summary>
    /// Gets per-I/O timeout.
    /// </summary>
    public TimeSpan IoTimeout { get; init; } = TimeSpan.FromSeconds(10);

    /// <summary>
    /// Applies timeout behavior to the specified operator.
    /// </summary>
    /// <param name="op">Operator to layer.</param>
    /// <returns>The layered operator instance.</returns>
    public Operator Apply(Operator op)
    {
        ArgumentNullException.ThrowIfNull(op);
        ObjectDisposedException.ThrowIf(op.IsInvalid, op);
        Validate();

        var result = NativeMethods.operator_layer_timeout(
            op,
            ToNanos(Timeout),
            ToNanos(IoTimeout)
        );

        return op.ApplyLayerResult(result);
    }

    private void Validate()
    {
        if (Timeout <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(nameof(Timeout), "Timeout must be greater than zero.");
        }

        if (IoTimeout <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(nameof(IoTimeout), "IoTimeout must be greater than zero.");
        }
    }

    private static ulong ToNanos(TimeSpan delay)
    {
        return checked((ulong)delay.Ticks * 100UL);
    }
}
