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
public sealed class CopyBehaviorTest : BehaviorTestBase
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    public CopyBehaviorTest(BehaviorOperatorFixture fixture)
        : base(fixture)
    {
    }

    [Fact]
    public void CopyBehavior_CreatesTargetWithSameContent()
    {
        if (!Supports(c => c.Copy && c.Read && c.Write))
        {
            return;
        }

        var sourcePath = NewPath("copy-source");
        var targetPath = NewPath("copy-target");
        var content = RandomBytes(256);

        Op.Write(sourcePath, content);
        var meta = Op.Copy(sourcePath, targetPath);

        if (meta.IsFile)
        {
            Assert.Equal((ulong)content.Length, meta.ContentLength);
        }

        Assert.Equal(content, Op.Read(targetPath));
    }

    [Fact]
    public async Task CopyBehavior_CreatesTargetWithSameContentAsync()
    {
        if (!Supports(c => c.Copy && c.Read && c.Write))
        {
            return;
        }

        var sourcePath = NewPath("copy-source-async");
        var targetPath = NewPath("copy-target-async");
        var content = RandomBytes(256);

        await Op.WriteAsync(sourcePath, content, cancellationToken: CT);
        var meta = await Op.CopyAsync(sourcePath, targetPath, cancellationToken: CT);

        if (meta.IsFile)
        {
            Assert.Equal((ulong)content.Length, meta.ContentLength);
        }

        Assert.Equal(content, await Op.ReadAsync(targetPath, cancellationToken: CT));
    }

    [Fact]
    public void CopyBehavior_IfNotExists_CopiesToNewPath()
    {
        if (!Supports(c => c.Copy && c.Read && c.Write && c.CopyWithIfNotExists))
        {
            return;
        }

        var sourcePath = NewPath("copy-if-not-exists-source");
        var targetPath = NewPath("copy-if-not-exists-target");
        var content = RandomBytes(256);

        Op.Write(sourcePath, content);
        Op.Copy(sourcePath, targetPath, new CopyOptions { IfNotExists = true });

        Assert.Equal(content, Op.Read(targetPath));
    }

    [Fact]
    public async Task CopyBehavior_IfNotExists_RejectsExistingTargetAsync()
    {
        if (!Supports(c => c.Copy && c.Read && c.Write && c.CopyWithIfNotExists))
        {
            return;
        }

        var sourcePath = NewPath("copy-if-not-exists-existing-source");
        var targetPath = NewPath("copy-if-not-exists-existing-target");
        var targetContent = RandomBytes(128);

        await Op.WriteAsync(sourcePath, RandomBytes(256), cancellationToken: CT);
        await Op.WriteAsync(targetPath, targetContent, cancellationToken: CT);

        var ex = await Assert.ThrowsAsync<OpenDALException>(
            () => Op.CopyAsync(sourcePath, targetPath, new CopyOptions { IfNotExists = true }, CT));
        Assert.Equal(ErrorCode.ConditionNotMatch, ex.Code);
        Assert.Equal(targetContent, await Op.ReadAsync(targetPath, cancellationToken: CT));
    }

    [Fact]
    public async Task CopyBehavior_IfNotChanged_RejectsStaleTargetMetadataAsync()
    {
        if (!Supports(c => c.Copy && c.Write && c.Stat && (c.CopyWithIfMatch || c.CopyWithIfVersionMatch)))
        {
            return;
        }

        var sourcePath = NewPath("copy-if-not-changed-source");
        var targetPath = NewPath("copy-if-not-changed-target");

        await Op.WriteAsync(sourcePath, RandomBytes(256), cancellationToken: CT);
        await Op.WriteAsync(targetPath, RandomBytes(128), cancellationToken: CT);
        var expected = await Op.StatAsync(targetPath, cancellationToken: CT);

        await Op.CopyAsync(sourcePath, targetPath, new CopyOptions { IfNotChanged = expected }, CT);

        var ex = await Assert.ThrowsAsync<OpenDALException>(
            () => Op.CopyAsync(sourcePath, targetPath, new CopyOptions { IfNotChanged = expected }, CT));
        Assert.Equal(ErrorCode.ConditionNotMatch, ex.Code);
    }
}
