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

public class OperatorInfoTest
{
    [Fact]
    public void OperatorInfo_MemoryConfig_ReturnsExpectedSchemeAndCapabilities()
    {
        var config = new MemoryServiceConfig
        {
            Root = "/opendal/",
        };

        using var op = new Operator(config);
        var info = op.Info;

        Assert.Equal("memory", info.Scheme);
        Assert.True(info.Capability.Read);
        Assert.True(info.Capability.Write);
    }

    [Fact]
    public void OperatorInfo_MemoryConfig_AbsentSizeLimitsAreNull()
    {
        using var op = new Operator(new MemoryServiceConfig());
        var capability = op.Info.Capability;

        // The memory service declares none of the size limits. They must
        // surface as null, not as a sentinel leaking through the FFI layer.
        Assert.Null(capability.WriteMultiMaxSize);
        Assert.Null(capability.WriteMultiMinSize);
        Assert.Null(capability.WriteTotalMaxSize);
        Assert.Null(capability.DeleteMaxSize);
        Assert.Null(capability.CopyMultiMaxSize);
        Assert.Null(capability.CopyMultiMinSize);
    }

    [Fact]
    public void OperatorInfo_FsConfig_ReturnsExpectedSchemeAndCapabilities()
    {
        var root = Path.Combine(Path.GetTempPath(), $"opendal-dotnet-info-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);

        try
        {
            var options = new Dictionary<string, string>
            {
                ["root"] = root,
            };

            using var op = new Operator("fs", options);
            var info = op.Info;

            Assert.Equal("fs", info.Scheme);
            Assert.True(info.Capability.Read);
            Assert.True(info.Capability.Write);
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }
}
