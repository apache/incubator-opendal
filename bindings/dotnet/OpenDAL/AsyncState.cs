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

using System.Collections.Concurrent;
using System.Diagnostics.CodeAnalysis;

internal static class AsyncStateRegistry
{
    private static long nextAsyncStateId;
    private static readonly ConcurrentDictionary<long, object> AsyncStates = new();

    public static long Register<T>(out AsyncState<T> state)
    {
        state = new AsyncState<T>();

        while (true)
        {
            var id = Interlocked.Increment(ref nextAsyncStateId);
            if (id == 0)
            {
                continue;
            }

            if (AsyncStates.TryAdd(id, state))
            {
                state.Context = id;
                return id;
            }
        }
    }

    public static void Unregister(long context)
    {
        AsyncStates.TryRemove(context, out _);
    }

    public static bool TryTake<TState>(long context, [NotNullWhen(true)] out TState? state) where TState : class
    {
        state = null;
        if (!AsyncStates.TryRemove(context, out var value))
        {
            return false;
        }

        state = value as TState;
        return state is not null;
    }
}

internal sealed class AsyncState<T>
{
    internal long Context { get; set; }

    public TaskCompletionSource<T> Completion { get; } = new(TaskCreationOptions.RunContinuationsAsynchronously);

    public CancellationTokenRegistration CancellationRegistration { get; private set; }

    public void BindCancellation(CancellationToken cancellationToken)
    {
        if (!cancellationToken.CanBeCanceled)
        {
            return;
        }

        var registration = cancellationToken.Register(static value =>
        {
            var current = (AsyncState<T>)value!;
            AsyncStateRegistry.Unregister(current.Context);
            current.Completion.TrySetCanceled();
        }, this);

        if (Completion.Task.IsCompleted)
        {
            registration.Dispose();
            return;
        }

        CancellationRegistration = registration;

        if (Completion.Task.IsCompleted)
        {
            CancellationRegistration.Dispose();
        }
    }
}