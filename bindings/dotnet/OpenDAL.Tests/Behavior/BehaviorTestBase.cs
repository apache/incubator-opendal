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

using System.Security.Cryptography;

namespace OpenDAL.Tests;

public abstract class BehaviorTestBase
{
    private readonly BehaviorOperatorFixture fixture;

    protected BehaviorTestBase(BehaviorOperatorFixture fixture)
    {
        this.fixture = fixture;
    }

    protected bool IsEnabled => fixture.IsEnabled;

    protected Operator Op => fixture.Op;

    protected Capability CapabilityBeforeOverrides => fixture.CapabilityBeforeOverrides;

    protected Capability Capability => fixture.Capability;

    protected bool IsService(string scheme)
    {
        return string.Equals(fixture.Scheme, scheme, StringComparison.OrdinalIgnoreCase);
    }

    protected bool Supports(Func<Capability, bool> predicate)
    {
        return IsEnabled && predicate(Capability);
    }

    protected static byte[] RandomBytes(int size)
    {
        var bytes = new byte[size];
        RandomNumberGenerator.Fill(bytes);
        return bytes;
    }

    protected string NewPath(string prefix)
    {
        return $"dotnet-behavior/{prefix}-{Guid.NewGuid():N}";
    }

    protected static bool IsMissingError(OpenDALException ex)
    {
        return ex.Code == ErrorCode.NotFound || ex.Code == ErrorCode.NotADirectory;
    }

}
