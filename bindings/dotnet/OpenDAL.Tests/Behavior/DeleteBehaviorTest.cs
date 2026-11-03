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
public sealed class DeleteBehaviorTest : BehaviorTestBase
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    public DeleteBehaviorTest(BehaviorOperatorFixture fixture)
        : base(fixture)
    {
    }

    [Fact]
    public void DeleteBehavior_RemovesObject()
    {
        if (!Supports(c => c.Delete && c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("delete");

        Op.Write(path, RandomBytes(12));
        Op.Delete(path);

        var ex = Assert.Throws<OpenDALException>(() => Op.Read(path));
        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public async Task DeleteBehavior_RemovesObjectAsync()
    {
        if (!Supports(c => c.Delete && c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("delete-async");

        await Op.WriteAsync(path, RandomBytes(12), cancellationToken: CT);
        await Op.DeleteAsync(path, cancellationToken: CT);

        var ex = await Assert.ThrowsAsync<OpenDALException>(() => Op.ReadAsync(path, cancellationToken: CT));
        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public void DeleteBehavior_DeletingMissingPath_IsAllowed()
    {
        if (!Supports(c => c.Delete))
        {
            return;
        }

        Op.Delete(NewPath("delete-missing"));
    }

    [Fact]
    public async Task DeleteBehavior_DeletingMissingPath_IsAllowedAsync()
    {
        if (!Supports(c => c.Delete))
        {
            return;
        }

        await Op.DeleteAsync(NewPath("delete-missing-async"), cancellationToken: CT);
    }

    [Fact]
    public async Task DeleteBehavior_WithVersion_RemovesVersionPermanentlyAsync()
    {
        if (!Supports(c => c.DeleteWithVersion && c.StatWithVersion && c.Write))
        {
            return;
        }

        var path = NewPath("delete-version");
        await Op.WriteAsync(path, RandomBytes(16), cancellationToken: CT);

        var version = (await Op.StatAsync(path, cancellationToken: CT)).Version;
        Assert.NotNull(version);

        await Op.DeleteAsync(path, cancellationToken: CT);
        var missing = await Assert.ThrowsAsync<OpenDALException>(() => Op.StatAsync(path, cancellationToken: CT));
        Assert.True(IsMissingError(missing));

        var archived = await Op.StatAsync(path, new StatOptions { Version = version }, CT);
        Assert.Equal(version, archived.Version);

        await Op.DeleteAsync(path, new DeleteOptions { Version = version }, CT);

        var gone = await Assert.ThrowsAsync<OpenDALException>(
            () => Op.StatAsync(path, new StatOptions { Version = version }, CT));
        Assert.Equal(ErrorCode.NotFound, gone.Code);
    }

    [Fact]
    public void DeleteBehavior_WithRecursive_RemovesTree()
    {
        if (!Supports(c => c.DeleteWithRecursive && c.Write && c.List))
        {
            return;
        }

        var dir = NewPath("delete-recursive") + "/";
        Op.Write($"{dir}a.txt", RandomBytes(16));
        Op.Write($"{dir}nested/b.txt", RandomBytes(16));

        Op.Delete(dir, new DeleteOptions { Recursive = true });

        Assert.Empty(Op.List(dir, new ListOptions { Recursive = true }));
    }

    [Fact]
    public void DeleteBehavior_WithVersionFromAnotherObject_IsAllowed()
    {
        if (!Supports(c => c.DeleteWithVersion && c.StatWithVersion && c.Write))
        {
            return;
        }

        var donor = NewPath("delete-version-donor");
        Op.Write(donor, RandomBytes(8));
        var foreignVersion = Op.Stat(donor).Version;
        Assert.NotNull(foreignVersion);

        var target = NewPath("delete-version-target");
        Op.Write(target, RandomBytes(8));

        Op.Delete(target, new DeleteOptions { Version = foreignVersion });
    }

    [Fact]
    public async Task DeleteBehavior_WithIfMatch_RemovesObjectIfMatchedAsync()
    {
        if (!Supports(c => c.DeleteWithIfMatch && c.Write && c.Stat))
        {
            return;
        }

        var path = NewPath("delete-if-match");
        await Op.WriteAsync(path, RandomBytes(16), cancellationToken: CT);

        var etag = (await Op.StatAsync(path, cancellationToken: CT)).ETag;
        Assert.NotNull(etag);

        await Op.DeleteAsync(path, new DeleteOptions { IfMatch = etag }, CT);

        var ex = await Assert.ThrowsAsync<OpenDALException>(() => Op.StatAsync(path, cancellationToken: CT));
        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public async Task DeleteBehavior_WithIfMatch_DoesNotRemoveObjectIfNotMatchedAsync()
    {
        if (!Supports(c => c.DeleteWithIfMatch && c.Write && c.Stat))
        {
            return;
        }

        var path = NewPath("delete-if-match-not-matched");
        await Op.WriteAsync(path, RandomBytes(16), cancellationToken: CT);

        var etag = (await Op.StatAsync(path, cancellationToken: CT)).ETag;
        Assert.NotNull(etag);

        var ex = await Assert.ThrowsAsync<OpenDALException>(
            () => Op.DeleteAsync(path, new DeleteOptions { IfMatch = "\"this-etag-does-not-match\"" }, CT));
        Assert.Equal(ErrorCode.ConditionNotMatch, ex.Code);

        var stat = await Op.StatAsync(path, cancellationToken: CT);
        Assert.Equal(etag, stat.ETag);
    }
}
