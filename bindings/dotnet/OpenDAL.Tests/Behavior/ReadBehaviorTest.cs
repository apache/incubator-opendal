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
public sealed class ReadBehaviorTest : BehaviorTestBase
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    public ReadBehaviorTest(BehaviorOperatorFixture fixture)
        : base(fixture)
    {
    }

    [Fact]
    public void ReadBehavior_ReadsWrittenData()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("read-sync");
        var content = RandomBytes(2048);

        Op.Write(path, content);
        var actual = Op.Read(path);

        Assert.Equal(content, actual);
    }

    [Fact]
    public async Task ReadBehavior_ReadsWrittenDataAsync()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("read-async");
        var content = RandomBytes(2048);

        await Op.WriteAsync(path, content, cancellationToken: CT);
        var actual = await Op.ReadAsync(path, cancellationToken: CT);

        Assert.Equal(content, actual);
    }

    [Fact]
    public void ReadBehavior_LargePayload_RoundTripsExactly()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        // Covers the whole path end to end at a size worth measuring. The chunk
        // walking itself is pinned by the unit tests in src/buffer.rs, which build
        // multi-part buffers a backend may or may not produce.
        const int defaultSize = 8 * 1024 * 1024;
        var size = (int)Math.Min(
            (ulong)defaultSize,
            Capability.WriteTotalMaxSize ?? ulong.MaxValue);
        var path = NewPath("read-large");
        var content = RandomBytes(size);

        Op.Write(path, content);
        var actual = Op.Read(path);

        Assert.Equal(content.Length, actual.Length);
        Assert.True(content.AsSpan().SequenceEqual(actual));
    }

    [Fact]
    public async Task ReadBehavior_LargePayload_RoundTripsExactlyAsync()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        const int defaultSize = 8 * 1024 * 1024;
        var size = (int)Math.Min(
            (ulong)defaultSize,
            Capability.WriteTotalMaxSize ?? ulong.MaxValue);
        var path = NewPath("read-large-async");
        var content = RandomBytes(size);

        await Op.WriteAsync(path, content, cancellationToken: CT);
        var actual = await Op.ReadAsync(path, cancellationToken: CT);

        Assert.Equal(content.Length, actual.Length);
        Assert.True(content.AsSpan().SequenceEqual(actual));
    }

    [Fact]
    public void ReadBehavior_WithRange_ReturnsExpectedBytes()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("read-range");
        var content = RandomBytes(512);

        Op.Write(path, content);
        var partial = Op.Read(path, new ReadOptions { Offset = 10, Length = 100 });

        Assert.Equal(content.AsSpan(10, 100).ToArray(), partial);
    }

    [Fact]
    public async Task ReadBehavior_WithRange_ReturnsExpectedBytesAsync()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("read-range-async");
        var content = RandomBytes(512);

        await Op.WriteAsync(path, content, cancellationToken: CT);
        var partial = await Op.ReadAsync(path, new ReadOptions { Offset = 10, Length = 100 }, CT);

        Assert.Equal(content.AsSpan(10, 100).ToArray(), partial);
    }

    [Fact]
    public void ReadBehavior_MissingPath_ReturnsNotFound()
    {
        if (!Supports(c => c.Read))
        {
            return;
        }

        var ex = Assert.Throws<OpenDALException>(() => Op.Read(NewPath("missing")));

        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public async Task ReadBehavior_MissingPath_ReturnsNotFoundAsync()
    {
        if (!Supports(c => c.Read))
        {
            return;
        }

        var ex = await Assert.ThrowsAsync<OpenDALException>(() => Op.ReadAsync(NewPath("missing-async"), cancellationToken: CT));

        Assert.True(IsMissingError(ex));
    }

    [Fact]
    public async Task ReadBehavior_ConsumeCallback_ReturnsConsumerResult()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("read-consume");
        var content = RandomBytes(100_000);
        await Op.WriteAsync(path, content, cancellationToken: CT);

        var length = 0L;
        var actual = Op.Read(path, sequence =>
        {
            length = sequence.Length;
            return sequence.ToArray();
        });

        Assert.Equal(content.Length, length);
        Assert.Equal(content, actual);

        var asyncActual = await Op.ReadAsync(path, sequence => sequence.ToArray(), cancellationToken: CT);
        Assert.Equal(content, asyncActual);
    }

    [Fact]
    public async Task ReadBehavior_ConsumeCallback_ChunkedPayloadRoundtrips()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        // Written in many small pieces, so backends that store the payload as
        // written hand the consumer a sequence spanning several segments.
        var path = NewPath("read-consume-chunked");
        var content = RandomBytes(200_000);

        await Op.WriteAsync(path, writer => FillInChunks(writer, content), cancellationToken: CT);

        var actual = await Op.ReadAsync(path, sequence => sequence.ToArray(), cancellationToken: CT);
        Assert.Equal(content, actual);

        static void FillInChunks(IBufferWriter<byte> writer, byte[] content)
        {
            var remaining = content.AsSpan();
            while (!remaining.IsEmpty)
            {
                var chunk = remaining[..Math.Min(999, remaining.Length)];
                chunk.CopyTo(writer.GetSpan(chunk.Length));
                writer.Advance(chunk.Length);
                remaining = remaining[chunk.Length..];
            }
        }
    }

    [Fact]
    public async Task ReadBehavior_ConsumeCallback_ParsesJsonDirectly()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("read-consume-json");
        var expected = new Dictionary<string, int[]>
        {
            ["values"] = Enumerable.Range(0, 5_000).ToArray(),
        };

        await Op.WriteAsync(path, writer =>
        {
            using var json = new System.Text.Json.Utf8JsonWriter(writer);
            System.Text.Json.JsonSerializer.Serialize(json, expected);
        }, cancellationToken: CT);

        var actual = await Op.ReadAsync(path, Deserialize, cancellationToken: CT);

        Assert.NotNull(actual);
        Assert.Equal(expected["values"], actual!["values"]);

        static Dictionary<string, int[]>? Deserialize(ReadOnlySequence<byte> sequence)
        {
            var reader = new System.Text.Json.Utf8JsonReader(sequence);
            return System.Text.Json.JsonSerializer.Deserialize<Dictionary<string, int[]>>(ref reader);
        }
    }

    [Fact]
    public async Task ReadBehavior_ConsumeCallback_ConsumerFailurePropagates()
    {
        if (!Supports(c => c.Read && c.Write))
        {
            return;
        }

        var path = NewPath("read-consume-throw");
        var content = RandomBytes(1024);
        await Op.WriteAsync(path, content, cancellationToken: CT);

        Assert.Throws<InvalidOperationException>(() => Op.Read<byte[]>(
            path, _ => throw new InvalidOperationException("consumer failed")));
        await Assert.ThrowsAsync<InvalidOperationException>(() => Op.ReadAsync<byte[]>(
            path, _ => throw new InvalidOperationException("consumer failed"), cancellationToken: CT));

        Assert.Equal(content, Op.Read(path));
    }

    [Fact]
    public async Task ReadBehavior_ConsumeCallback_RejectsNullConsumer()
    {
        if (!Supports(c => c.Read))
        {
            return;
        }

        var path = NewPath("read-consume-arguments");

        Assert.Throws<ArgumentNullException>(() => Op.Read(
            path, (Func<ReadOnlySequence<byte>, byte[]>)null!));
        await Assert.ThrowsAsync<ArgumentNullException>(() => Op.ReadAsync(
            path, (Func<ReadOnlySequence<byte>, byte[]>)null!, cancellationToken: CT));
    }

    [Fact]
    public async Task ReadBehavior_WithSuffixLength_ReturnsTailAsync()
    {
        if (!Supports(c => c.Read && c.Write && c.ReadWithSuffix))
        {
            return;
        }

        var path = NewPath("read-suffix");
        var content = RandomBytes(1024);

        await Op.WriteAsync(path, content, cancellationToken: CT);
        var tail = await Op.ReadAsync(path, new ReadOptions { SuffixLength = 257 }, CT);

        Assert.Equal(content.AsSpan(content.Length - 257).ToArray(), tail);
    }

    [Fact]
    public void ReadBehavior_WithSuffixLengthLargerThanFile_ReturnsWholeFile()
    {
        if (!Supports(c => c.Read && c.Write && c.ReadWithSuffix))
        {
            return;
        }

        var path = NewPath("read-suffix-larger");
        var content = RandomBytes(1024);

        Op.Write(path, content);
        var tail = Op.Read(path, new ReadOptions { SuffixLength = content.Length * 2 });

        Assert.Equal(content, tail);
    }

    [Fact]
    public void ReadBehavior_WithSuffixLengthAndOffset_RejectsOptions()
    {
        if (!Supports(c => c.Read))
        {
            return;
        }

        var ex = Assert.Throws<ArgumentException>(
            () => Op.Read(NewPath("read-suffix-offset"), new ReadOptions { Offset = 1, SuffixLength = 1 }));
        Assert.Equal("SuffixLength", ex.ParamName);
    }
}
