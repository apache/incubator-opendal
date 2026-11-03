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
public sealed class RemoveAllBehaviorTest : BehaviorTestBase
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    public RemoveAllBehaviorTest(BehaviorOperatorFixture fixture)
        : base(fixture)
    {
    }

    [Fact]
    public void RemoveAllBehavior_RemovesRecursiveTree()
    {
        if (!Supports(c => c.DeleteWithRecursive && c.Write && c.List))
        {
            return;
        }

        var dir = NewPath("remove-all") + "/";

        Op.Write($"{dir}a.txt", RandomBytes(16));
        Op.Write($"{dir}nested/b.txt", RandomBytes(16));
        Op.RemoveAll(dir);

        var entries = Op.List(dir, new Options.ListOptions { Recursive = true });
        Assert.Empty(entries);
    }

    [Fact]
    public async Task RemoveAllBehavior_RemovesRecursiveTreeAsync()
    {
        if (!Supports(c => c.DeleteWithRecursive && c.Write && c.List))
        {
            return;
        }

        var dir = NewPath("remove-all-async") + "/";

        await Op.WriteAsync($"{dir}a.txt", RandomBytes(16), cancellationToken: CT);
        await Op.WriteAsync($"{dir}nested/b.txt", RandomBytes(16), cancellationToken: CT);
        await Op.RemoveAllAsync(dir, CT);

        var entries = await Op.ListAsync(dir, new Options.ListOptions { Recursive = true }, CT);
        Assert.Empty(entries);
    }
}
