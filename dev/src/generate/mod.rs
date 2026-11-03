// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

mod capability;
mod docs;
mod dotnet;
mod gcs_grpc;
mod java;
mod metadata;
mod options;
mod parser;
mod python;

use crate::workspace_dir;
use anyhow::Result;

pub fn run(language: &str) -> Result<()> {
    let workspace_dir = workspace_dir();

    if language == "gcs-grpc" {
        return gcs_grpc::generate(workspace_dir);
    }

    let mut services = parser::Services::new();

    // Old layout: core/core/src/services/<name>/config.rs (e.g. memory)
    let old_services_path = workspace_dir.join("core/core/src/services");
    if old_services_path.exists() {
        services.extend(parser::parse(&old_services_path.to_string_lossy())?);
    }

    // New layout: core/services/<name>/src/config.rs (most services)
    let new_services_path = workspace_dir.join("core/services");
    if new_services_path.exists() {
        services.extend(parser::parse(&new_services_path.to_string_lossy())?);
    }

    match language {
        "java" => java::generate(workspace_dir, services),
        "python" | "py" => python::generate(workspace_dir, services),
        "dotnet" | "cs" => dotnet::generate(workspace_dir, services),
        "docs" => docs::generate(workspace_dir, services),
        _ => anyhow::bail!("unsupported language: {}", language),
    }
}
