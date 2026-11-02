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

use crate::types::builder::{Builder, Configurator};
use crate::types::{IntoOperatorUri, OperatorUri};
use crate::{Error, ErrorKind, Operator, Result};
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

/// Factory signature used to construct [`Operator`] from a URI and extra options.
pub type OperatorFactory = fn(&OperatorUri) -> Result<Operator>;

/// Global registry that maps schemes to [`OperatorFactory`] functions.
#[derive(Debug)]
pub struct OperatorRegistry {
    factories: Mutex<HashMap<String, OperatorFactory>>,
}

impl OperatorRegistry {
    /// Get the global registry.
    pub fn get() -> &'static Self {
        /// The global registry used by [`Operator::from_uri`].
        ///
        /// OpenDAL always registers the `memory` service because OpenDAL uses it
        /// extensively in unit tests and it has no dependencies.
        ///
        /// The `opendal` facade crate registers other optional services. See
        /// `core/src/lib.rs` in the facade crate for details.
        static OPERATOR_REGISTRY: LazyLock<OperatorRegistry> = LazyLock::new(|| {
            let factories = Mutex::new(HashMap::default());
            let registry = OperatorRegistry { factories };
            crate::services::register_memory_service(&registry);
            registry
        });

        &OPERATOR_REGISTRY
    }

    /// Register a builder for the given scheme.
    pub fn register<B: Builder>(&self, scheme: &str) {
        let key = scheme.to_ascii_lowercase();
        let mut guard = self
            .factories
            .lock()
            .expect("operator registry mutex poisoned");
        guard.insert(key, factory::<B::Config>);
    }

    /// Return the set of schemes currently registered.
    ///
    /// For the global registry returned by [`OperatorRegistry::get`], this is
    /// exactly the set of schemes that [`Operator::from_uri`] can construct, as
    /// determined by the compiled-in `services-*` features.
    ///
    /// ```
    /// use opendal_core::OperatorRegistry;
    ///
    /// let schemes = OperatorRegistry::get().schemes();
    /// assert!(schemes.contains("memory"));
    /// ```
    pub fn schemes(&self) -> HashSet<String> {
        self.factories
            .lock()
            .expect("operator registry mutex poisoned")
            .keys()
            .cloned()
            .collect()
    }

    /// Load an [`Operator`] via the factory registered for the URI's scheme.
    pub fn load(&self, uri: impl IntoOperatorUri) -> Result<Operator> {
        let parsed = uri.into_operator_uri()?;
        let scheme = parsed.scheme();

        let factory = {
            let guard = self
                .factories
                .lock()
                .expect("operator registry mutex poisoned");

            match guard.get(scheme).copied() {
                Some(factory) => factory,
                None => {
                    // Collect the available schemes from the guard we already hold
                    // to avoid re-locking the mutex (which would self-deadlock).
                    let mut available: Vec<String> = guard.keys().cloned().collect();
                    available.sort();
                    return Err(
                        Error::new(ErrorKind::Unsupported, "scheme is not registered")
                            .with_context("scheme", scheme.to_string())
                            .with_context("available", available.join(", ")),
                    );
                }
            }
        };

        factory(&parsed)
    }
}

/// Factory adapter that builds an operator from a configurator type.
fn factory<C: Configurator>(uri: &OperatorUri) -> Result<Operator> {
    let cfg = C::from_uri(uri)?;
    Operator::from_config(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_registry_exposes_memory_scheme() {
        let schemes = OperatorRegistry::get().schemes();
        assert!(schemes.contains("memory"));
    }

    #[test]
    fn unregistered_scheme_error_lists_available_schemes() {
        let err = OperatorRegistry::get()
            .load("no-such-scheme://bucket/path")
            .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::Unsupported);

        let msg = err.to_string();
        assert!(msg.contains("scheme is not registered"));
        assert!(msg.contains("no-such-scheme"));
        assert!(msg.contains("available"));
        assert!(msg.contains("memory"));
    }
}
