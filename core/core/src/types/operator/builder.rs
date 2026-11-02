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

use std::sync::Arc;

use crate::layers::*;
use crate::raw::*;
use crate::types::IntoOperatorUri;
use crate::*;

impl Operator {
    /// Create a new operator with input builder.
    ///
    /// OpenDAL calls [`Builder::build`] internally and returns a ready-to-use
    /// [`Operator`].
    ///
    /// # Examples
    ///
    /// ```
    /// use opendal_core::services::Memory;
    /// use opendal_core::{Operator, Result};
    ///
    /// # fn example() -> Result<()> {
    /// let op = Operator::new(Memory::default())?;
    /// # let _ = op;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<B: Builder>(ab: B) -> Result<Operator> {
        let service = Arc::new(ab.build()?) as Servicer;
        Ok(Operator::from_parts(OperationContext::default(), service)
            .layer(ErrorContextLayer)
            .layer(SimulateLayer::default())
            .layer(CompleteLayer)
            .layer(CorrectnessCheckLayer))
    }

    /// Create a new operator from given config.
    ///
    /// The config is converted into its corresponding builder and then follows
    /// the same construction path as [`Operator::new`].
    ///
    /// # Examples
    ///
    /// ```
    /// use opendal_core::services::MemoryConfig;
    /// use opendal_core::{Operator, Result};
    ///
    /// # fn example() -> Result<()> {
    /// let op = Operator::from_config(MemoryConfig::default())?;
    /// # let _ = op;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_config<C: Configurator>(cfg: C) -> Result<Operator> {
        let builder = cfg.into_builder();
        Operator::new(builder)
    }

    /// Create a new operator from given iterator.
    ///
    /// The service builder type is selected statically by `B`, while config
    /// values are parsed from the provided key-value iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// use opendal_core::services::Memory;
    /// use opendal_core::{Operator, Result};
    ///
    /// # fn example() -> Result<()> {
    /// let cfg: HashMap<String, String> = HashMap::new();
    /// let op = Operator::from_iter::<Memory>(cfg)?;
    /// # let _ = op;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_iter<B: Builder>(
        iter: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Operator> {
        let builder = B::Config::from_iter(iter)?.into_builder();
        Operator::new(builder)
    }

    /// Create a new operator by parsing configuration from a URI.
    ///
    /// The URI scheme is resolved through [`OperatorRegistry`], so this only
    /// works for services registered in the current build.
    ///
    /// # Examples
    ///
    /// ```
    /// use opendal_core::{Operator, Result};
    ///
    /// # fn example() -> Result<()> {
    /// let op = Operator::from_uri("memory://localhost/")?;
    /// # let _ = op;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_uri(uri: impl IntoOperatorUri) -> Result<Operator> {
        OperatorRegistry::get().load(uri)
    }

    /// Create a new operator via given scheme and iterator of config value.
    ///
    /// This is the scheme-driven variant of [`Operator::from_iter`]. It
    /// delegates to the registry, allowing callers to construct an operator
    /// without naming the builder type at compile time.
    ///
    /// # Examples
    ///
    /// ```
    /// use opendal_core::services;
    /// use opendal_core::{Operator, Result};
    ///
    /// # fn example() -> Result<()> {
    /// let cfg = Vec::<(String, String)>::new();
    /// let op = Operator::via_iter(services::MEMORY_SCHEME, cfg)?;
    /// # let _ = op;
    /// # Ok(())
    /// # }
    /// ```
    pub fn via_iter(
        scheme: impl AsRef<str>,
        iter: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Operator> {
        Operator::from_uri((scheme.as_ref(), iter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services;

    #[test]
    fn via_iter_delegates_to_registry() {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])
            .expect("memory scheme should be registered");
        let _ = op;
    }

    #[test]
    fn new_returns_finished_operator() {
        let op = Operator::new(services::Memory::default()).expect("memory builder should build");
        assert_eq!(op.info().scheme(), services::MEMORY_SCHEME);
    }
}
