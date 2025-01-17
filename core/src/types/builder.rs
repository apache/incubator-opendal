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

use http::Uri;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::raw::*;
use crate::*;
use types::operator::{OperatorFactory, OperatorRegistry};

/// Builder is used to set up underlying services.
///
/// This trait allows the developer to define a builder struct that can:
///
/// - build a service via builder style API.
/// - configure in-memory options like `http_client` or `customized_credential_load`.
///
/// Usually, users don't need to use or import this trait directly, they can use `Operator` API instead.
///
/// For example:
///
/// ```
/// # use anyhow::Result;
/// use opendal::services::Fs;
/// use opendal::Operator;
/// async fn test() -> Result<()> {
///     // Create fs backend builder.
///     let mut builder = Fs::default().root("/tmp");
///
///     // Build an `Operator` to start operating the storage.
///     let op: Operator = Operator::new(builder)?.finish();
///
///     Ok(())
/// }
/// ```
pub trait Builder: Default + 'static {
    /// Associated scheme for this builder. It indicates what underlying service is.
    const SCHEME: Scheme;
    /// Associated configuration for this builder.
    type Config: Configurator;

    /// Consume the accessor builder to build a service.
    fn build(self) -> Result<impl Access>;

    /// Register this builder in the given registry.
    fn register_in_registry(registry: &mut OperatorRegistry) {
        let operator_factory: OperatorFactory = |uri, options| {
            let builder = Self::Config::from_uri(uri, options)?.into_builder();
            Ok(Operator::new(builder)?.finish())
        };
        registry.register(Self::SCHEME.into_static(), operator_factory)
    }
}

/// Dummy implementation of builder
impl Builder for () {
    const SCHEME: Scheme = Scheme::Custom("dummy");
    type Config = ();

    fn build(self) -> Result<impl Access> {
        Ok(())
    }
}

/// Configurator is used to configure the underlying service.
///
/// This trait allows the developer to define a configuration struct that can:
///
/// - deserialize from an iterator like hashmap or vector.
/// - convert into a service builder and finally build the underlying services.
///
/// Usually, users don't need to use or import this trait directly, they can use `Operator` API instead.
///
/// For example:
///
/// ```
/// # use anyhow::Result;
/// use std::collections::HashMap;
///
/// use opendal::services::MemoryConfig;
/// use opendal::Operator;
/// async fn test() -> Result<()> {
///     let mut cfg = MemoryConfig::default();
///     cfg.root = Some("/".to_string());
///
///     // Build an `Operator` to start operating the storage.
///     let op: Operator = Operator::from_config(cfg)?.finish();
///
///     Ok(())
/// }
/// ```
///
/// Some service builder might contain in memory options like `http_client` . Users can call
/// `into_builder` to convert the configuration into a builder instead.
///
/// ```
/// # use anyhow::Result;
/// use std::collections::HashMap;
///
/// use opendal::raw::HttpClient;
/// use opendal::services::S3Config;
/// use opendal::Configurator;
/// use opendal::Operator;
///
/// async fn test() -> Result<()> {
///     let mut cfg = S3Config::default();
///     cfg.root = Some("/".to_string());
///     cfg.bucket = "test".to_string();
///
///     let builder = cfg.into_builder();
///     let builder = builder.http_client(HttpClient::new()?);
///
///     // Build an `Operator` to start operating the storage.
///     let op: Operator = Operator::new(builder)?.finish();
///
///     Ok(())
/// }
/// ```
pub trait Configurator: Serialize + DeserializeOwned + Debug + 'static {
    /// Associated builder for this configuration.
    type Builder: Builder;

    /// Deserialize from an iterator.
    ///
    /// This API is provided by opendal, developer should not implement it.
    fn from_iter(iter: impl IntoIterator<Item = (String, String)>) -> Result<Self> {
        let cfg = ConfigDeserializer::new(iter.into_iter().collect());

        Self::deserialize(cfg).map_err(|err| {
            Error::new(ErrorKind::ConfigInvalid, "failed to deserialize config").set_source(err)
        })
    }

    /// TODO: document this.
    fn from_uri(uri: &str, options: impl IntoIterator<Item = (String, String)>) -> Result<Self> {
        let parsed_uri = uri.parse::<Uri>().map_err(|err| {
            Error::new(ErrorKind::ConfigInvalid, "uri is invalid")
                .with_context("uri", uri)
                .set_source(err)
        })?;

        // TODO: I have some doubts about this default implementation
        // It was inspired from https://github.com/apache/opendal/blob/52c96bb8e8cb4d024ccab1f415c4756447c726dd/bin/ofs/src/main.rs#L60
        // Parameters should be specified in uri's query param. Example: 'fs://?root=<directory>'
        // this is very similar to https://github.com/apache/opendal/blob/52c96bb8e8cb4d024ccab1f415c4756447c726dd/bin/ofs/README.md?plain=1#L45
        let query_pairs = parsed_uri.query().map(query_pairs).unwrap_or_default();

        // TODO: should we log a warning if the query_pairs vector is empty?

        // TODO: we are not interpreting the host or path params
        // the `let op = Operator::from_uri("fs:///tmp/test", vec![])?;` statement from the RFC wont work.
        // instead we should use `let op = Operator::from_uri("fs://?root=/tmp/test", vec![])?;` as done
        // in `ofs`. The `fs` service should override this default implementation if it wants to use the host or path params?

        let merged_options = query_pairs.into_iter().chain(options);

        Self::from_iter(merged_options)
    }

    /// Convert this configuration into a service builder.
    fn into_builder(self) -> Self::Builder;
}

impl Configurator for () {
    type Builder = ();

    fn into_builder(self) -> Self::Builder {}
}
