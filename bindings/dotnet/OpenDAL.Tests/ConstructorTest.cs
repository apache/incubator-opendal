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

using OpenDAL.ServiceConfig;

namespace OpenDAL.Tests;

public class ConstructorTest
{
    [Fact]
    public void Constructor_SchemeIsEmpty_ThrowsArgumentException()
    {
        var ex = Assert.Throws<ArgumentException>(() => new Operator(""));

        Assert.Contains("scheme", ex.Message, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void Constructor_SchemeIsWhitespace_ThrowsArgumentException()
    {
        var ex = Assert.Throws<ArgumentException>(() => new Operator("   "));

        Assert.Contains("scheme", ex.Message, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void Dictionary_NullKey_ThrowsArgumentNullException()
    {
        Assert.Throws<ArgumentNullException>(() =>
        {
            var options = new Dictionary<string, string>
            {
                [null!] = "value",
            };
            _ = options;
        });
    }

    [Fact]
    public void Constructor_OptionValueIsNull_ThrowsConfigInvalid()
    {
        var options = new Dictionary<string, string>
        {
            ["root"] = null!,
        };

        var ex = Assert.Throws<OpenDALException>(() => new Operator("memory", options));

        Assert.Equal(ErrorCode.ConfigInvalid, ex.Code);
        Assert.Contains("value of option root is null", ex.Message);
    }

    [Fact]
    public void Constructor_InvalidScheme_ThrowsUnsupported()
    {
        var ex = Assert.Throws<OpenDALException>(() => new Operator("invalid-service"));

        Assert.Equal(ErrorCode.Unsupported, ex.Code);
    }

    [Fact]
    public void Constructor_OptionsMapProvided_ConstructsOperator()
    {
        var options = new Dictionary<string, string>
        {
            ["root"] = "/tmp",
        };

        using var op = new Operator("memory", options);

        Assert.NotEqual(IntPtr.Zero, op.DangerousGetHandle());
    }

    [Fact]
    public void Constructor_ServiceConfigProvided_ConstructsOperator()
    {
        var config = new MemoryServiceConfig
        {
            Root = "/tmp",
        };

        using var op = new Operator(config);

        Assert.NotEqual(IntPtr.Zero, op.DangerousGetHandle());
    }

    [Fact]
    public void Constructor_OptionsDictionaryCallsBackIntoOpenDAL_KeepsOptionsIntact()
    {
        using var other = new Operator("memory");
        other.Write("file", new byte[] { 1, 2, 3 });
        var options = new ReentrantDictionary(other, new Dictionary<string, string> { ["root"] = "/reentrant/" });

        using var op = new Operator("memory", options);

        Assert.Equal("/reentrant/", op.Info.Root);
    }

    /// <summary>
    /// Serializes other options on the same thread while it is being enumerated.
    /// </summary>
    private sealed class ReentrantDictionary(Operator other, Dictionary<string, string> inner)
        : IReadOnlyDictionary<string, string>
    {
        public string this[string key] => inner[key];

        public IEnumerable<string> Keys => inner.Keys;

        public IEnumerable<string> Values => inner.Values;

        public int Count => inner.Count;

        public bool ContainsKey(string key) => inner.ContainsKey(key);

        public bool TryGetValue(string key, out string value) => inner.TryGetValue(key, out value!);

        public IEnumerator<KeyValuePair<string, string>> GetEnumerator()
        {
            foreach (var entry in inner)
            {
                other.Read("file", new OpenDAL.Options.ReadOptions { Offset = 1, Length = 1 });
                yield return entry;
            }
        }

        System.Collections.IEnumerator System.Collections.IEnumerable.GetEnumerator() => GetEnumerator();
    }
}
