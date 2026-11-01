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
public sealed class CreateDirBehaviorTest : BehaviorTestBase
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    public CreateDirBehaviorTest(BehaviorOperatorFixture fixture)
        : base(fixture)
    {
    }

    [Fact]
    public void CreateDirBehavior_CreatesDirectoryEntry()
    {
        if (!Supports(c => c.CreateDir && c.Stat))
        {
            return;
        }

        var dirPath = NewPath("mkdir") + "/";

        Op.CreateDir(dirPath);
        var meta = Op.Stat(dirPath);

        Assert.True(meta.IsDir);
    }

    [Fact]
    public async Task CreateDirBehavior_CreatesDirectoryEntryAsync()
    {
        if (!Supports(c => c.CreateDir && c.Stat))
        {
            return;
        }

        var dirPath = NewPath("mkdir-async") + "/";

        await Op.CreateDirAsync(dirPath, CT);
        var meta = await Op.StatAsync(dirPath, null, cancellationToken: CT);

        Assert.True(meta.IsDir);
    }
}
