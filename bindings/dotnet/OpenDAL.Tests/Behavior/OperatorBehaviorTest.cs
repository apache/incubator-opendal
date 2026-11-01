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

[Collection("BehaviorOperator")]
public sealed class OperatorBehaviorTest : BehaviorTestBase
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    public OperatorBehaviorTest(BehaviorOperatorFixture fixture)
        : base(fixture)
    {
    }

    [Fact]
    public void OperatorBehavior_Duplicate_SharesBackendState()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        using var duplicated = Op.Duplicate();

        var path = NewPath("duplicate");
        var content = RandomBytes(32);

        duplicated.Write(path, content);
        Assert.Equal(content, Op.Read(path));
    }

    [Fact]
    public async Task OperatorBehavior_CancelAfterDispatch_DoesNotBreakSubsequentOperations()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("cancel-seed");
        await Op.WriteAsync(path, System.Text.Encoding.UTF8.GetBytes("seed-content"), cancellationToken: CT);

        using (var writeCts = new CancellationTokenSource())
        {
            var writeTask = Op.WriteAsync(NewPath("cancel-write"), [1, 2, 3, 4], cancellationToken: writeCts.Token);
            writeCts.Cancel();

            try
            {
                await writeTask;
            }
            catch (OperationCanceledException)
            {
            }
        }

        using (var readCts = new CancellationTokenSource())
        {
            var readTask = Op.ReadAsync(path, cancellationToken: readCts.Token);
            readCts.Cancel();

            try
            {
                _ = await readTask;
            }
            catch (OperationCanceledException)
            {
            }
        }

        var stableRead = await Op.ReadAsync(path, cancellationToken: CT);
        Assert.Equal("seed-content", System.Text.Encoding.UTF8.GetString(stableRead));
    }
}
