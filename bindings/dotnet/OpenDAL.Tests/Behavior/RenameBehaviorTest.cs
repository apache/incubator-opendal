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

using OpenDAL.Options;

namespace OpenDAL.Tests;

[Collection("BehaviorOperator")]
public sealed class RenameBehaviorTest : BehaviorTestBase
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    public RenameBehaviorTest(BehaviorOperatorFixture fixture)
        : base(fixture)
    {
    }

    [Fact]
    public void RenameBehavior_MovesContentToTargetPath()
    {
        if (!Supports(c => c.Rename && c.Read && c.Write))
        {
            return;
        }

        var sourcePath = NewPath("rename-source");
        var targetPath = NewPath("rename-target");
        var content = RandomBytes(111);

        Op.Write(sourcePath, content);
        Op.Rename(sourcePath, targetPath);

        Assert.Equal(content, Op.Read(targetPath));
        var ex = Assert.Throws<OpenDALException>(() => Op.Read(sourcePath));
        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public async Task RenameBehavior_MovesContentToTargetPathAsync()
    {
        if (!Supports(c => c.Rename && c.Read && c.Write))
        {
            return;
        }

        var sourcePath = NewPath("rename-source-async");
        var targetPath = NewPath("rename-target-async");
        var content = RandomBytes(111);

        await Op.WriteAsync(sourcePath, content, cancellationToken: CT);
        await Op.RenameAsync(sourcePath, targetPath, cancellationToken: CT);

        Assert.Equal(content, await Op.ReadAsync(targetPath, cancellationToken: CT));
        var ex = await Assert.ThrowsAsync<OpenDALException>(() => Op.ReadAsync(sourcePath, cancellationToken: CT));
        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public void RenameBehavior_IfNotExists_MovesToNewPath()
    {
        if (!Supports(c => c.Rename && c.Read && c.Write && c.RenameWithIfNotExists))
        {
            return;
        }

        var sourcePath = NewPath("rename-if-not-exists-source");
        var targetPath = NewPath("rename-if-not-exists-target");
        var content = RandomBytes(111);

        Op.Write(sourcePath, content);
        Op.Rename(sourcePath, targetPath, new RenameOptions { IfNotExists = true });

        Assert.Equal(content, Op.Read(targetPath));
        var ex = Assert.Throws<OpenDALException>(() => Op.Read(sourcePath));
        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public async Task RenameBehavior_IfNotExists_RejectsExistingTargetAsync()
    {
        if (!Supports(c => c.Rename && c.Read && c.Write && c.RenameWithIfNotExists))
        {
            return;
        }

        var sourcePath = NewPath("rename-if-not-exists-existing-source");
        var targetPath = NewPath("rename-if-not-exists-existing-target");
        var sourceContent = RandomBytes(111);
        var targetContent = RandomBytes(222);

        await Op.WriteAsync(sourcePath, sourceContent, cancellationToken: CT);
        await Op.WriteAsync(targetPath, targetContent, cancellationToken: CT);

        var ex = await Assert.ThrowsAsync<OpenDALException>(
            () => Op.RenameAsync(sourcePath, targetPath, new RenameOptions { IfNotExists = true }, CT));
        Assert.Equal(ErrorCode.ConditionNotMatch, ex.Code);
        Assert.Equal(sourceContent, await Op.ReadAsync(sourcePath, cancellationToken: CT));
        Assert.Equal(targetContent, await Op.ReadAsync(targetPath, cancellationToken: CT));
    }
}
