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

namespace OpenDAL;

/// <summary>
/// Represents metadata of an OpenDAL operator.
/// </summary>
public sealed class OperatorInfo
{
    /// <summary>
    /// Gets the scheme of this operator.
    /// </summary>
    public string Scheme { get; }

    /// <summary>
    /// Gets the configured root of this operator.
    /// </summary>
    public string Root { get; }

    /// <summary>
    /// Gets the configured name of this operator.
    /// </summary>
    public string Name { get; }

    /// <summary>
    /// Gets the capability of this operator.
    /// </summary>
    public Capability Capability { get; }

    internal OperatorInfo(string scheme, string root, string name, Capability capability)
    {
        Scheme = scheme;
        Root = root;
        Name = name;
        Capability = capability;
    }
}
