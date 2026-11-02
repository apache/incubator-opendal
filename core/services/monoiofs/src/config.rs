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

use opendal_core::*;
use serde::Deserialize;
use serde::Serialize;

use super::backend::MonoiofsBuilder;

/// Config for monoiofs services support.
#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct MonoiofsConfig {
    /// The Root of this backend.
    ///
    /// All operations will happen under this root.
    ///
    /// Builder::build will return error if not set.
    pub root: Option<String>,
}

impl Configurator for MonoiofsConfig {
    type Builder = MonoiofsBuilder;

    fn from_uri(uri: &OperatorUri) -> Result<Self> {
        let mut map = uri.options().clone();

        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        MonoiofsBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_uri_sets_root() -> Result<()> {
        let uri = OperatorUri::new("monoiofs:///tmp", Vec::<(String, String)>::new())?;

        let cfg = MonoiofsConfig::from_uri(&uri)?;
        assert_eq!(cfg.root.as_deref(), Some("tmp"));
        Ok(())
    }
}
