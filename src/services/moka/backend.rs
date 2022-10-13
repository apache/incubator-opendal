// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io::Result;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use log::info;
use moka::future::Cache;

use crate::adapters::kv;
use crate::Scheme;
use crate::{Accessor, AccessorCapability};

/// Builder for moka backend
#[derive(Default, Debug)]
pub struct Builder {
    /// Name for this cache instance.
    name: Option<String>,
    /// Sets the max capacity of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::max_capacity`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.max_capacity)
    max_capacity: Option<u64>,
    /// Sets the time to live of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::time_to_live`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.time_to_live)
    time_to_live: Option<Duration>,
    /// Sets the time to idle of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::time_to_idle`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.time_to_idle)
    time_to_idle: Option<Duration>,
}

impl Builder {
    pub(crate) fn from_iter(it: impl Iterator<Item = (String, String)>) -> Self {
        let mut builder = Builder::default();
        for (k, v) in it {
            let v = v.as_str();
            match k.as_ref() {
                "name" => builder.name(v),
                "max_capacity" => match v.parse::<u64>() {
                    Ok(v) => builder.max_capacity(v),
                    _ => continue,
                },
                "time_to_live" => match v.parse::<u64>() {
                    Ok(v) => builder.time_to_live(Duration::from_secs(v)),
                    _ => continue,
                },
                "time_to_idle" => match v.parse::<u64>() {
                    Ok(v) => builder.time_to_idle(Duration::from_secs(v)),
                    _ => continue,
                },
                _ => continue,
            };
        }
        builder
    }

    /// Name for this cache instance.
    pub fn name(&mut self, v: &str) -> &mut Self {
        if !v.is_empty() {
            self.name = Some(v.to_owned());
        }
        self
    }

    /// Sets the max capacity of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::max_capacity`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.max_capacity)
    pub fn max_capacity(&mut self, v: u64) -> &mut Self {
        if v != 0 {
            self.max_capacity = Some(v);
        }
        self
    }

    /// Sets the time to live of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::time_to_live`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.time_to_live)
    pub fn time_to_live(&mut self, v: Duration) -> &mut Self {
        if !v.is_zero() {
            self.time_to_live = Some(v);
        }
        self
    }

    /// Sets the time to idle of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::time_to_idle`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.time_to_idle)
    pub fn time_to_idle(&mut self, v: Duration) -> &mut Self {
        if !v.is_zero() {
            self.time_to_idle = Some(v);
        }
        self
    }

    /// Consume builder to build a moka backend.
    pub fn build(&mut self) -> Result<impl Accessor> {
        info!("backend build started: {:?}", &self);

        let mut builder = Cache::builder();
        if let Some(v) = &self.name {
            builder = builder.name(v);
        }
        if let Some(v) = self.max_capacity {
            builder = builder.max_capacity(v)
        }
        if let Some(v) = self.time_to_live {
            builder = builder.time_to_live(v)
        }
        if let Some(v) = self.time_to_idle {
            builder = builder.time_to_idle(v)
        }

        info!("backend build finished: {:?}", &self);
        Ok(Backend::new(Adapter {
            inner: builder.build(),
            next_id: Arc::new(AtomicU64::new(1)),
        }))
    }
}

/// Backend is used to serve `Accessor` support in moka.
pub type Backend = kv::Backend<Adapter>;

#[derive(Debug, Clone)]
pub struct Adapter {
    inner: Cache<Vec<u8>, Vec<u8>>,
    next_id: Arc<AtomicU64>,
}

#[async_trait]
impl kv::Adapter for Adapter {
    fn metadata(&self) -> kv::Metadata {
        kv::Metadata::new(
            Scheme::Moka,
            self.inner.name().unwrap_or("moka"),
            AccessorCapability::Read | AccessorCapability::Write,
        )
    }

    async fn next_id(&self) -> Result<u64> {
        Ok(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match self.inner.get(key) {
            None => Ok(None),
            Some(bs) => Ok(Some(bs)),
        }
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.inner.insert(key.to_vec(), value.to_vec()).await;

        Ok(())
    }

    async fn delete(&self, key: &[u8]) -> Result<()> {
        self.inner.invalidate(key).await;

        Ok(())
    }
}
