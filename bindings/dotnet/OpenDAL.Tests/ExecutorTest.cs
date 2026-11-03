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

namespace OpenDAL.Tests;

public class ExecutorTest
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    [Fact]
    public void CreateExecutor_InvalidCores_ThrowsArgumentOutOfRangeException()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => new Executor(0));
    }

    [Fact]
    public void ReadWrite_WithDedicatedExecutor_RoundTripsSuccessfully()
    {
        using var executor = new Executor(1);
        using var op = new Operator("memory", executor: executor);
        var content = System.Text.Encoding.UTF8.GetBytes("executor-content");

        op.Write("executor-sync", content);
        var read = op.Read("executor-sync");

        Assert.Equal(content, read);
    }

    [Fact]
    public async Task ReadWriteAsync_WithDedicatedExecutor_RoundTripsSuccessfully()
    {
        using var executor = new Executor(1);
        using var op = new Operator("memory", executor: executor);
        var content = System.Text.Encoding.UTF8.GetBytes("executor-async-content");

        await op.WriteAsync("executor-async", content, cancellationToken: CT);
        var read = await op.ReadAsync("executor-async", cancellationToken: CT);

        Assert.Equal(content, read);
    }

    [Fact]
    public void Construct_WithDisposedExecutor_ThrowsObjectDisposedException()
    {
        var executor = new Executor(1);
        executor.Dispose();

        Assert.Throws<ObjectDisposedException>(() => new Operator("memory", executor: executor));
    }

    [Fact]
    public async Task Operator_OutlivesDisposedExecutor_KeepsWorking()
    {
        var executor = new Executor(1);
        using var op = new Operator("memory", executor: executor);
        var content = new byte[] { 1, 2, 3 };

        // The operator binds the runtime at construction, so disposing the
        // executor handle afterwards must not tear the runtime down.
        executor.Dispose();

        op.Write("outlives-executor-sync", content);
        Assert.Equal(content, op.Read("outlives-executor-sync"));

        await op.WriteAsync("outlives-executor-async", content, cancellationToken: CT);
        Assert.Equal(content, await op.ReadAsync("outlives-executor-async", cancellationToken: CT));
    }

    [Fact]
    public void ReadWrite_WithProcessorCountThreads_RoundTripsSuccessfully()
    {
        var threads = Math.Max(1, Environment.ProcessorCount);
        using var executor = new Executor(threads);
        using var op = new Operator("memory", executor: executor);
        var content = System.Text.Encoding.UTF8.GetBytes($"executor-threads-{threads}");

        op.Write("executor-processor-sync", content);
        var read = op.Read("executor-processor-sync");

        Assert.Equal(content, read);
    }

    [Fact]
    public async Task ReadWriteAsync_WithProcessorCountThreads_ParallelOperationsSucceed()
    {
        var threads = Math.Max(1, Environment.ProcessorCount);
        var operationCount = Math.Min(threads * 2, 64);

        using var executor = new Executor(threads);
        using var op = new Operator("memory", executor: executor);

        var tasks = Enumerable.Range(0, operationCount).Select(async i =>
        {
            var path = $"executor-processor-async-{i}";
            var content = System.Text.Encoding.UTF8.GetBytes($"executor-content-{i}");

            await op.WriteAsync(path, content, cancellationToken: CT);
            var read = await op.ReadAsync(path, cancellationToken: CT);

            Assert.Equal(content, read);
        });

        await Task.WhenAll(tasks);
    }
}
