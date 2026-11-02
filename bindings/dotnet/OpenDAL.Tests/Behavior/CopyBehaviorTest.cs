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
        var meta = await Op.CopyAsync(sourcePath, targetPath, CT);

        if (meta.IsFile)
        {
            Assert.Equal((ulong)content.Length, meta.ContentLength);
        }

        Assert.Equal(content, await Op.ReadAsync(targetPath, cancellationToken: CT));
    }
}
