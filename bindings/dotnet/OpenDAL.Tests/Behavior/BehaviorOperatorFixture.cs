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

using System.Collections;
using OpenDAL.Layer;

namespace OpenDAL.Tests;

public sealed class BehaviorOperatorFixture : IDisposable
{
    private const string CapabilityOverridesEnv = "OPENDAL_TEST_CAPABILITY_OVERRIDES";
    private readonly Operator? op;
    private readonly Capability capabilityBeforeOverrides;
    private readonly Capability capability;

    public string? Scheme { get; }

    public BehaviorOperatorFixture()
    {
        Scheme = Environment.GetEnvironmentVariable("OPENDAL_TEST");
        if (string.IsNullOrWhiteSpace(Scheme))
        {
            return;
        }

        var scheme = Scheme.ToLowerInvariant().Replace('_', '-');
        var options = BuildConfigFromEnvironment(Scheme);

        // Align with other bindings: isolate behavior tests with random roots by default.
        if (!IsRandomRootDisabled())
        {
            var baseRoot = options.TryGetValue("root", out var root) && !string.IsNullOrWhiteSpace(root)
                ? root!
                : "/";
            options["root"] = BuildRandomRoot(baseRoot);
        }

        op = new Operator(scheme, options).WithLayer(new RetryLayer());
        capabilityBeforeOverrides = op.Info.Capability;
        var capabilityOverrides = Environment.GetEnvironmentVariable(CapabilityOverridesEnv);
        if (!string.IsNullOrWhiteSpace(capabilityOverrides))
        {
            op = op.WithLayer(new CapabilityOverrideLayer(capabilityOverrides));
        }

        capability = op.Info.Capability;
    }

    public bool IsEnabled => op is not null;

    public Operator Op => op ?? throw new InvalidOperationException("Behavior operator is not initialized.");

    public Capability CapabilityBeforeOverrides => capabilityBeforeOverrides;

    public Capability Capability => capability;

    public void Dispose()
    {
        op?.Dispose();
    }

    private static Dictionary<string, string> BuildConfigFromEnvironment(string service)
    {
        var variables = Environment.GetEnvironmentVariables();
        var prefix = $"opendal_{service.ToLowerInvariant()}_";
        var config = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);

        foreach (DictionaryEntry entry in variables)
        {
            var key = entry.Key?.ToString();
            var value = entry.Value?.ToString();
            if (string.IsNullOrWhiteSpace(key) || value is null)
            {
                continue;
            }

            var normalized = key.ToLowerInvariant();
            if (!normalized.StartsWith(prefix, StringComparison.Ordinal))
            {
                continue;
            }

            config[normalized[prefix.Length..]] = value;
        }

        return config;
    }

    private static bool IsRandomRootDisabled()
    {
        return string.Equals(
            Environment.GetEnvironmentVariable("OPENDAL_DISABLE_RANDOM_ROOT"),
            "true",
            StringComparison.OrdinalIgnoreCase);
    }

    private static string BuildRandomRoot(string baseRoot)
    {
        var trimmed = baseRoot.Trim();
        if (trimmed.Length == 0)
        {
            trimmed = "/";
        }

        if (!trimmed.EndsWith('/'))
        {
            trimmed += "/";
        }

        return $"{trimmed}{Guid.NewGuid():N}/";
    }
}
