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

use std::fmt::Debug;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::raw::*;
use crate::types::OperatorUri;
use crate::*;

/// OpenDAL uses `Builder` to set up a service.
///
/// A `Builder` allows developers to:
///
/// - Build a service through a builder-style API.
/// - Configure in-memory options, such as `customized_credential_load`.
///
/// Users can usually use the [`Operator`] API instead of importing and using
/// `Builder` directly. For example:
///
/// ```
/// # use anyhow::Result;
/// use opendal_core::services::Memory;
/// use opendal_core::Operator;
/// async fn test() -> Result<()> {
///     // Create a memory backend builder.
///     let builder = Memory::default();
///
///     // Build an `Operator` to start operating the storage.
///     let op: Operator = Operator::new(builder)?;
///
///     Ok(())
/// }
/// ```
pub trait Builder: Default + 'static {
    /// Associated configuration.
    type Config: Configurator;

    /// Consume the builder to build a service.
    fn build(self) -> Result<impl Service>;
}

/// Dummy implementation of builder
impl Builder for () {
    type Config = ();

    fn build(self) -> Result<impl Service> {
        Ok(())
    }
}

/// OpenDAL uses `Configurator` to configure a service.
///
/// This trait allows the developer to define a configuration struct that can:
///
/// - Deserialize from an iterator such as a hash map or vector.
/// - Convert into a service builder and then build a concrete service.
///
/// Users can usually use the [`Operator`] API instead of importing and using
/// `Configurator` directly. For example:
///
/// ```
/// # use anyhow::Result;
/// use std::collections::HashMap;
///
/// use opendal_core::services::MemoryConfig;
/// use opendal_core::Operator;
/// async fn test() -> Result<()> {
///     let mut cfg = MemoryConfig::default();
///     cfg.root = Some("/".to_string());
///
///     // Build an `Operator` to start operating the storage.
///     let op: Operator = Operator::from_config(cfg)?;
///
///     Ok(())
/// }
/// ```
pub trait Configurator: Serialize + DeserializeOwned + Debug + 'static {
    /// Associated builder for this configuration.
    type Builder: Builder;

    /// Build configuration from a parsed URI with merged options.
    fn from_uri(_uri: &OperatorUri) -> Result<Self> {
        Err(Error::new(ErrorKind::Unsupported, "uri is not supported"))
    }

    /// Deserialize from an iterator.
    ///
    /// OpenDAL provides this method; service developers should not need to override it.
    fn from_iter(iter: impl IntoIterator<Item = (String, String)>) -> Result<Self> {
        let cfg = ConfigDeserializer::new(iter.into_iter().collect());

        Self::deserialize(cfg).map_err(|err| {
            Error::new(ErrorKind::ConfigInvalid, "failed to deserialize config").set_source(err)
        })
    }

    /// Convert this configuration into a service builder.
    fn into_builder(self) -> Self::Builder;
}

impl Configurator for () {
    type Builder = ();

    fn into_builder(self) -> Self::Builder {}
}
