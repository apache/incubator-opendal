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
public sealed class PresignBehaviorTest : BehaviorTestBase
{
    private static CancellationToken CT => TestContext.Current.CancellationToken;

    public PresignBehaviorTest(BehaviorOperatorFixture fixture)
        : base(fixture)
    {
    }

    [Fact]
    public async Task PresignBehavior_ReadWriteStatDelete_AreGeneratedWhenSupported()
    {
        if (!IsEnabled)
        {
            return;
        }

        var path = NewPath("presign");
        var expire = TimeSpan.FromMinutes(10);

        if (Capability.PresignRead)
        {
            var req = await Op.PresignReadAsync(path, expire, CT);
            Assert.False(string.IsNullOrWhiteSpace(req.Method));
            Assert.False(string.IsNullOrWhiteSpace(req.Uri));
        }

        if (Capability.PresignWrite)
        {
            var req = await Op.PresignWriteAsync(path, expire, CT);
            Assert.False(string.IsNullOrWhiteSpace(req.Method));
            Assert.False(string.IsNullOrWhiteSpace(req.Uri));
        }

        if (Capability.PresignStat)
        {
            var req = await Op.PresignStatAsync(path, expire, CT);
            Assert.False(string.IsNullOrWhiteSpace(req.Method));
            Assert.False(string.IsNullOrWhiteSpace(req.Uri));
        }

        if (Capability.PresignDelete)
        {
            var req = await Op.PresignDeleteAsync(path, expire, CT);
            Assert.False(string.IsNullOrWhiteSpace(req.Method));
            Assert.False(string.IsNullOrWhiteSpace(req.Uri));
        }
    }
}
