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

use serde::Deserialize;
use serde::Serialize;

use super::backend::GitBuilder;

/// Configuration for Git service support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct GitConfig {
    /// Git repository URL or local path.
    ///
    /// Examples:
    ///
    /// - `https://github.com/apache/opendal.git`
    /// - `https://huggingface.co/hf-internal-testing/tiny-random-bert`
    /// - `file:///path/to/repository.git`
    ///
    /// This is required.
    pub repository: Option<String>,

    /// Git reference to resolve.
    ///
    /// The value can be a branch, tag, commit hash, or `HEAD`. The reference is
    /// resolved once when the operator first reads the repository. Later
    /// operations use the same immutable snapshot and do not refresh it.
    ///
    /// The default is `HEAD`.
    pub reference: Option<String>,

    /// Root path within the repository.
    ///
    /// All operations are relative to this path. The default is `/`.
    pub root: Option<String>,

    /// Username used for the Git fetch and same-origin LFS requests.
    ///
    /// Credentials are not sent to a cross-origin LFS download action. This is
    /// optional.
    pub username: Option<String>,

    /// Password or personal access token used for the Git fetch and
    /// same-origin LFS requests.
    ///
    /// Credentials are not sent to a cross-origin LFS download action. This is
    /// optional.
    pub password: Option<String>,

    /// Whether to resolve Git LFS pointer files.
    ///
    /// When enabled, the service detects Git LFS pointers and streams the
    /// referenced object. When disabled, the pointer file itself is returned.
    /// The default is `true`.
    pub resolve_lfs: Option<bool>,
}

impl std::fmt::Debug for GitConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitConfig")
            .field("repository", &self.repository)
            .field("reference", &self.reference)
            .field("root", &self.root)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("resolve_lfs", &self.resolve_lfs)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for GitConfig {
    type Builder = GitBuilder;

    fn into_builder(self) -> Self::Builder {
        GitBuilder { config: self }
    }
}
