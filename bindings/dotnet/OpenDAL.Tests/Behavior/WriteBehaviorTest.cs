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

using System.Buffers;
using OpenDAL.Options;

namespace OpenDAL.Tests;

[Collection("BehaviorOperator")]
public sealed class WriteBehaviorTest : BehaviorTestBase
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    public WriteBehaviorTest(BehaviorOperatorFixture fixture)
        : base(fixture)
    {
    }

    [Fact]
    public void WriteBehavior_BasicRoundtrip()
    {
        if (!Supports(c => c.Write && c.Read))
        {
            return;
        }

        var path = NewPath("write");
        var content = RandomBytes(1024);

        Op.Write(path, content);
        var actual = Op.Read(path);

        Assert.Equal(content, actual);
    }

    [Fact]
    public async Task WriteBehavior_BasicRoundtripAsync()
    {
        if (!Supports(c => c.Write && c.Read))
        {
            return;
        }

        var path = NewPath("write-async");
        var content = RandomBytes(1024);

        await Op.WriteAsync(path, content, cancellationToken: CT);
        var actual = await Op.ReadAsync(path, cancellationToken: CT);

        Assert.Equal(content, actual);
    }

    [Fact]
    public void WriteBehavior_SyncWriteCopiesTheSource()
    {
        if (!Supports(c => c.Write && c.Read))
        {
            return;
        }

        // Guards against the write path lending the caller's array to the
        // backend: with aliasing, mutating the source after Write changes
        // what in-process backends hand back on the next read, and the
        // stored pointer dangles once the GC collects the array.
        var path = NewPath("write-copies-source");
        var content = RandomBytes(1024);
        var expected = (byte[])content.Clone();

        Op.Write(path, content);
        content.AsSpan().Fill(0xEE);

        Assert.Equal(expected, Op.Read(path));
    }

    [Fact]
    public async Task WriteBehavior_LargeContentRoundtripAsync()
    {
        if (!Supports(c => c.Write && c.Read))
        {
            return;
        }

        var path = NewPath("write-async-large");
        var content = RandomBytes(256 * 1024);

        await Op.WriteAsync(path, content, cancellationToken: CT);
        var actual = await Op.ReadAsync(path, cancellationToken: CT);

        Assert.Equal(content, actual);
    }

    [Fact]
    public async Task WriteBehavior_FillCallback_Roundtrips()
    {
        if (!Supports(c => c.Write && c.Read))
        {
            return;
        }

        var path = NewPath("write-fill");
        var content = RandomBytes(256 * 1024);

        await Op.WriteAsync(path, writer =>
        {
            content.CopyTo(writer.GetSpan(content.Length));
            writer.Advance(content.Length);
        }, sizeHint: content.Length, cancellationToken: CT);

        var actual = await Op.ReadAsync(path, cancellationToken: CT);
        Assert.Equal(content, actual);

        var syncPath = NewPath("write-fill-sync");
        Op.Write(syncPath, writer =>
        {
            content.CopyTo(writer.GetSpan(content.Length));
            writer.Advance(content.Length);
        }, sizeHint: content.Length);

        Assert.Equal(content, Op.Read(syncPath));
    }

    [Fact]
    public async Task WriteBehavior_FillCallback_ChunkedFillRoundtrips()
    {
        if (!Supports(c => c.Write && c.Read))
        {
            return;
        }

        // Produced in many small pieces and large enough that the writer must
        // grow while filling, which is how serializers without a known output
        // size behave.
        var path = NewPath("write-fill-chunked");
        var content = RandomBytes(200_000);

        await Op.WriteAsync(path, writer => FillInChunks(writer, content), cancellationToken: CT);

        var actual = await Op.ReadAsync(path, cancellationToken: CT);
        Assert.Equal(content, actual);

        static void FillInChunks(IBufferWriter<byte> writer, byte[] content)
        {
            var remaining = content.AsSpan();
            while (!remaining.IsEmpty)
            {
                var chunk = remaining[..Math.Min(1000, remaining.Length)];
                chunk.CopyTo(writer.GetSpan(chunk.Length));
                writer.Advance(chunk.Length);
                remaining = remaining[chunk.Length..];
            }
        }
    }

    [Fact]
    public async Task WriteBehavior_FillCallback_SerializesJsonDirectly()
    {
        if (!Supports(c => c.Write && c.Read))
        {
            return;
        }

        var path = NewPath("write-fill-json");
        var expected = new Dictionary<string, int[]>
        {
            ["values"] = Enumerable.Range(0, 10_000).ToArray(),
        };

        await Op.WriteAsync(path, writer =>
        {
            using var json = new System.Text.Json.Utf8JsonWriter(writer);
            System.Text.Json.JsonSerializer.Serialize(json, expected);
        }, cancellationToken: CT);

        var actual = System.Text.Json.JsonSerializer.Deserialize<Dictionary<string, int[]>>(
            await Op.ReadAsync(path, cancellationToken: CT));
        Assert.NotNull(actual);
        Assert.Equal(expected["values"], actual!["values"]);
    }

    [Fact]
    public async Task WriteBehavior_FillCallback_FillFailureWritesNothing()
    {
        if (!Supports(c => c.Write && c.Stat))
        {
            return;
        }

        var path = NewPath("write-fill-throw");

        await Assert.ThrowsAsync<InvalidOperationException>(() => Op.WriteAsync(
            path, _ => throw new InvalidOperationException("producer failed"), cancellationToken: CT));

        var ex = Assert.Throws<OpenDALException>(() => Op.Stat(path));
        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public void WriteBehavior_FillCallback_SyncFillFailureWritesNothing()
    {
        if (!Supports(c => c.Write && c.Stat))
        {
            return;
        }

        var path = NewPath("write-fill-throw-sync");

        Assert.Throws<InvalidOperationException>(() => Op.Write(
            path, _ => throw new InvalidOperationException("producer failed")));

        var ex = Assert.Throws<OpenDALException>(() => Op.Stat(path));
        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public async Task WriteBehavior_FillCallback_PreCanceledTokenWritesNothing()
    {
        if (!Supports(c => c.Write && c.Stat))
        {
            return;
        }

        var path = NewPath("write-fill-pre-canceled");
        using var cts = new CancellationTokenSource();
        cts.Cancel();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => Op.WriteAsync(
            path, _ => { }, cancellationToken: cts.Token));

        var ex = Assert.Throws<OpenDALException>(() => Op.Stat(path));
        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public async Task WriteBehavior_EmptyContentRoundtrips()
    {
        if (!Supports(c => c.Write && c.Read && c.WriteCanEmpty))
        {
            return;
        }

        var path = NewPath("write-empty");
        Op.Write(path, Array.Empty<byte>());
        Assert.Empty(Op.Read(path));

        var asyncPath = NewPath("write-empty-async");
        await Op.WriteAsync(asyncPath, Array.Empty<byte>(), cancellationToken: CT);
        Assert.Empty(await Op.ReadAsync(asyncPath, cancellationToken: CT));
    }

    [Fact]
    public async Task WriteBehavior_FillCallback_RejectsInvalidArguments()
    {
        if (!Supports(c => c.Write))
        {
            return;
        }

        var path = NewPath("write-fill-arguments");

        await Assert.ThrowsAsync<ArgumentNullException>(() => Op.WriteAsync(
            path, (Action<IBufferWriter<byte>>)null!, cancellationToken: CT));
        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(() => Op.WriteAsync(
            path, _ => { }, sizeHint: -1, cancellationToken: CT));
    }

    [Fact]
    public void WriteBehavior_IfNotExists_RejectsOverwrite()
    {
        if (!Supports(c => c.Write && c.Read && c.WriteWithIfNotExists))
        {
            return;
        }

        var path = NewPath("write-if-not-exists");
        var first = RandomBytes(128);
        var second = RandomBytes(64);

        Op.Write(path, first);

        var ex = Assert.Throws<OpenDALException>(() =>
            Op.Write(path, second, new WriteOptions { IfNotExists = true }));

        Assert.Contains(ex.Code, new[] { ErrorCode.ConditionNotMatch, ErrorCode.AlreadyExists });
        Assert.Equal(first, Op.Read(path));
    }

    [Fact]
    public async Task WriteBehavior_IfNotExists_RejectsOverwriteAsync()
    {
        if (!Supports(c => c.Write && c.Read && c.WriteWithIfNotExists))
        {
            return;
        }

        var path = NewPath("write-if-not-exists-async");
        var first = RandomBytes(128);
        var second = RandomBytes(64);

        await Op.WriteAsync(path, first, cancellationToken: CT);

        var ex = await Assert.ThrowsAsync<OpenDALException>(() =>
            Op.WriteAsync(path, second, new WriteOptions { IfNotExists = true }, CT));

        Assert.Contains(ex.Code, new[] { ErrorCode.ConditionNotMatch, ErrorCode.AlreadyExists });
        Assert.Equal(first, await Op.ReadAsync(path, cancellationToken: CT));
    }

    [Fact]
    public void WriteBehavior_Append_AppendsWhenSupported()
    {
        if (!Supports(c => c.Write && c.Read && c.WriteCanAppend))
        {
            return;
        }

        var path = NewPath("write-append");
        Op.Write(path, System.Text.Encoding.UTF8.GetBytes("a"), new WriteOptions { Append = true });
        Op.Write(path, System.Text.Encoding.UTF8.GetBytes("b"), new WriteOptions { Append = true });

        Assert.Equal("ab", System.Text.Encoding.UTF8.GetString(Op.Read(path)));
    }

    [Fact]
    public async Task WriteBehavior_Append_AppendsWhenSupportedAsync()
    {
        if (!Supports(c => c.Write && c.Read && c.WriteCanAppend))
        {
            return;
        }

        var path = NewPath("write-append-async");
        await Op.WriteAsync(path, System.Text.Encoding.UTF8.GetBytes("a"), new WriteOptions { Append = true }, CT);
        await Op.WriteAsync(path, System.Text.Encoding.UTF8.GetBytes("b"), new WriteOptions { Append = true }, CT);

        Assert.Equal("ab", System.Text.Encoding.UTF8.GetString(await Op.ReadAsync(path, cancellationToken: CT)));
    }

    [Fact]
    public void WriteBehavior_ReturnsMetadataOfWrittenObject()
    {
        if (!Supports(c => c.Write))
        {
            return;
        }

        var path = NewPath("write-returns-metadata");
        var content = RandomBytes(512);

        var meta = Op.Write(path, content);

        Assert.Equal((ulong)content.Length, meta.ContentLength);
    }

    [Fact]
    public async Task WriteBehavior_ReturnsMetadataOfWrittenObjectAsync()
    {
        if (!Supports(c => c.Write))
        {
            return;
        }

        var path = NewPath("write-returns-metadata-async");
        var content = RandomBytes(512);

        var meta = await Op.WriteAsync(path, content, cancellationToken: CT);

        Assert.Equal((ulong)content.Length, meta.ContentLength);
    }

    [Fact]
    public async Task WriteBehavior_FillCallback_ReturnsMetadataAsync()
    {
        if (!Supports(c => c.Write))
        {
            return;
        }

        var path = NewPath("write-fill-returns-metadata");
        var content = RandomBytes(300);

        var meta = await Op.WriteAsync(path, writer => writer.Write(content), cancellationToken: CT);

        Assert.Equal((ulong)content.Length, meta.ContentLength);
    }

    [Fact]
    public void WriteBehavior_WithUserMetadata_RoundtripsThroughStat()
    {
        if (!Supports(c => c.Write && c.Stat && c.WriteWithUserMetadata))
        {
            return;
        }

        var path = NewPath("write-user-metadata");
        var userMetadata = new Dictionary<string, string>
        {
            ["location"] = "everywhere",
        };

        Op.Write(path, RandomBytes(64), new WriteOptions { UserMetadata = userMetadata });
        var meta = Op.Stat(path);

        Assert.NotNull(meta.UserMetadata);
        Assert.Equal("everywhere", meta.UserMetadata["location"]);
    }
}
