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
public sealed class StreamBehaviorTest : BehaviorTestBase
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    public StreamBehaviorTest(BehaviorOperatorFixture fixture)
        : base(fixture)
    {
    }

    [Fact]
    public void StreamBehavior_Roundtrip_Works()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("stream-sync");
        var content = RandomBytes(64);

        using (var output = Op.OpenWriteStream(path))
        {
            output.Write(content, 0, content.Length);
            output.Flush();
        }

        using var input = Op.OpenReadStream(path);
        var buffer = new byte[content.Length];
        var read = input.Read(buffer, 0, buffer.Length);

        Assert.Equal(content.Length, read);
        Assert.Equal(content, buffer);
    }

    [Fact]
    public async Task StreamBehavior_RoundtripAsync_Works()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("stream-async");
        var content = RandomBytes(64);

        using (var output = Op.OpenWriteStream(path))
        {
            await output.WriteAsync(content, 0, content.Length, CT);
            await output.FlushAsync(CT);
        }

        using var input = Op.OpenReadStream(path);
        var buffer = new byte[content.Length];
        var read = await input.ReadAsync(buffer, 0, buffer.Length, CT);

        Assert.Equal(content.Length, read);
        Assert.Equal(content, buffer);
    }

    [Fact]
    public async Task StreamBehavior_LargeRoundtripAsync_Works()
    {
        if (!Supports(c => c.Read && c.Write && c.WriteCanMulti))
        {
            return;
        }

        // Large enough to cross the output stream's internal buffer many times
        // and to come back over several chunks, so both async paths make
        // repeated native round-trips instead of completing on the first call.
        var path = NewPath("stream-async-large");
        var content = RandomBytes(200_000);

        using (var output = Op.OpenWriteStream(path, bufferSize: 8 * 1024))
        {
            await output.WriteAsync(content.AsMemory(), CT);
            await output.FlushAsync(CT);
        }

        using var input = Op.OpenReadStream(path);
        var actual = new byte[content.Length];
        var filled = 0;
        while (filled < actual.Length)
        {
            var read = await input.ReadAsync(
                actual.AsMemory(filled, Math.Min(7_777, actual.Length - filled)), CT);
            if (read == 0)
            {
                break;
            }

            filled += read;
        }

        Assert.Equal(content.Length, filled);
        Assert.Equal(content, actual);
        Assert.Equal(0, await input.ReadAsync(actual.AsMemory(0, 1), CT));
    }

    [Fact]
    public void StreamBehavior_Complete_PersistsAndSealsTheStream()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("stream-complete");
        var content = RandomBytes(256);

        using var output = Op.OpenWriteStream(path);
        output.Write(content, 0, content.Length);
        output.Complete();

        Assert.Equal(content, Op.Read(path));
        Assert.Throws<InvalidOperationException>(() => output.Write(content, 0, 1));
        Assert.Throws<InvalidOperationException>(output.Complete);
    }

    [Fact]
    public async Task StreamBehavior_CompleteAsync_PersistsViaAwaitUsing()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("stream-complete-async");
        var content = RandomBytes(4 * 1024);

        await using (var output = Op.OpenWriteStream(path, bufferSize: 8 * 1024))
        {
            await output.WriteAsync(content.AsMemory(), CT);
            await output.CompleteAsync(CT);
        }

        Assert.Equal(content, await Op.ReadAsync(path, cancellationToken: CT));
    }

    [Fact]
    public async Task StreamBehavior_DisposeAsync_ClosesBestEffort()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("stream-dispose-async");
        var content = RandomBytes(4096);

        await using (var output = Op.OpenWriteStream(path))
        {
            await output.WriteAsync(content.AsMemory(), CT);
        }

        Assert.Equal(content, await Op.ReadAsync(path, cancellationToken: CT));
    }

    [Fact]
    public void StreamBehavior_OpenReadStream_WithRange_ReadsSelectedSlice()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("stream-range");
        Op.Write(path, System.Text.Encoding.UTF8.GetBytes("0123456789"));

        using var input = Op.OpenReadStream(path, new ReadOptions
        {
            Offset = 3,
            Length = 4,
        });

        var buffer = new byte[8];
        var read = input.Read(buffer, 0, buffer.Length);

        Assert.Equal(4, read);
        Assert.Equal("3456", System.Text.Encoding.UTF8.GetString(buffer, 0, read));
    }

    [Fact]
    public void StreamBehavior_OpenReadStream_WithIfMatch_AppliesCondition()
    {
        if (!Supports(c => c.Read && c.Write && c.Stat && c.ReadWithIfMatch))
        {
            return;
        }

        var path = NewPath("stream-if-match");
        var content = RandomBytes(64);
        Op.Write(path, content);
        var etag = Op.Stat(path).ETag;
        Assert.NotNull(etag);

        var ex = Assert.Throws<OpenDALException>(() =>
        {
            using var mismatched = Op.OpenReadStream(path, new ReadOptions { IfMatch = "\"invalid-etag\"" });
            mismatched.ReadByte();
        });
        Assert.Equal(ErrorCode.ConditionNotMatch, ex.Code);

        using var input = Op.OpenReadStream(path, new ReadOptions { IfMatch = etag });
        using var actual = new MemoryStream();
        input.CopyTo(actual);

        Assert.Equal(content, actual.ToArray());
    }

    [Fact]
    public void StreamBehavior_OpenReadStream_WithVersion_ReadsSpecifiedVersion()
    {
        if (!Supports(c => c.Read && c.Write && c.Stat && c.ReadWithVersion))
        {
            return;
        }

        var path = NewPath("stream-version");
        var first = RandomBytes(64);
        Op.Write(path, first);
        var version = Op.Stat(path).Version;
        Assert.NotNull(version);

        Op.Write(path, RandomBytes(64));

        using var input = Op.OpenReadStream(path, new ReadOptions { Version = version });
        using var actual = new MemoryStream();
        input.CopyTo(actual);

        Assert.Equal(first, actual.ToArray());
    }

    [Fact]
    public void StreamBehavior_OpenReadStream_IfMatchUnsupported_SurfacesUnsupported()
    {
        if (!Supports(c => c.Read && c.Write && !c.ReadWithIfMatch)
            || CapabilityBeforeOverrides.ReadWithIfMatch)
        {
            return;
        }

        var path = NewPath("stream-if-match-unsupported");
        Op.Write(path, RandomBytes(16));

        // An Unsupported error proves the condition was handed to the reader;
        // a stream that dropped the option would read the data without complaint.
        var ex = Assert.Throws<OpenDALException>(() =>
        {
            using var input = Op.OpenReadStream(path, new ReadOptions { IfMatch = "\"any-etag\"" });
            input.ReadByte();
        });

        Assert.Equal(ErrorCode.Unsupported, ex.Code);
    }

    [Fact]
    public void StreamBehavior_OpenReadStream_VersionUnsupported_SurfacesUnsupported()
    {
        if (!Supports(c => c.Read && c.Write && !c.ReadWithVersion)
            || CapabilityBeforeOverrides.ReadWithVersion)
        {
            return;
        }

        var path = NewPath("stream-version-unsupported");
        Op.Write(path, RandomBytes(16));

        var ex = Assert.Throws<OpenDALException>(() =>
        {
            using var input = Op.OpenReadStream(path, new ReadOptions { Version = "any-version" });
            input.ReadByte();
        });

        Assert.Equal(ErrorCode.Unsupported, ex.Code);
    }
}
