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

use asyncband::pool::ManageObject;
use asyncband::pool::ObjectStatus;
use asyncband::pool::bounded;
use etcd_client::Client;
use etcd_client::ConnectOptions;
use etcd_client::KvClient;
use std::fmt::Debug;
use std::sync::Arc;

use opendal_core::raw::*;
use opendal_core::{Buffer, Error, ErrorKind, Result};

pub mod constants {
    pub const DEFAULT_ETCD_ENDPOINTS: &str = "http://127.0.0.1:2379";
}

#[derive(Clone)]
pub struct Manager {
    endpoints: Vec<String>,
    options: ConnectOptions,
}

impl ManageObject for Manager {
    type Object = Client;
    type Error = Error;

    async fn create(&self) -> Result<Self::Object, Self::Error> {
        Client::connect(self.endpoints.clone(), Some(self.options.clone()))
            .await
            .map_err(format_etcd_error)
    }

    async fn is_recyclable(
        &self,
        o: &mut Self::Object,
        _: &ObjectStatus,
    ) -> Result<(), Self::Error> {
        match o.status().await {
            Ok(_) => Ok(()),
            Err(err) => Err(format_etcd_error(err)),
        }
    }
}

#[derive(Clone)]
pub struct EtcdCore {
    endpoints: Vec<String>,
    options: ConnectOptions,
    max_decoding_message_size: Option<usize>,
    client: Arc<bounded::Pool<Manager>>,
}

impl Debug for EtcdCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EtcdCore")
            .field("endpoints", &self.endpoints.join(","))
            .field("options", &self.options.clone())
            .field("max_decoding_message_size", &self.max_decoding_message_size)
            .finish_non_exhaustive()
    }
}

impl EtcdCore {
    pub fn new(
        endpoints: Vec<String>,
        options: ConnectOptions,
        max_decoding_message_size: Option<usize>,
    ) -> Self {
        let client = bounded::Pool::new(
            bounded::PoolConfig::new(64),
            Manager {
                endpoints: endpoints.clone(),
                options: options.clone(),
            },
        );

        Self {
            endpoints,
            options,
            max_decoding_message_size,
            client,
        }
    }

    pub async fn conn(&self) -> Result<bounded::Object<Manager>> {
        let fut = self.client.get();

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                Err(Error::new(ErrorKind::Unexpected, "connection request: timeout").set_temporary())
            }
            result = fut => match result {
                Ok(conn) => Ok(conn),
                Err(err) => Err(err),
            }
        }
    }

    pub async fn has_prefix(&self, prefix: &str) -> Result<bool> {
        let client = self.conn().await?;
        let get_options = Some(
            etcd_client::GetOptions::new()
                .with_prefix()
                .with_keys_only()
                .with_limit(1),
        );
        let resp = self
            .kv_client(&client)
            .get(prefix, get_options)
            .await
            .map_err(format_etcd_error)?;
        Ok(!resp.kvs().is_empty())
    }

    /// Builds a KV client from `conn` with the configured response decoding limit applied.
    ///
    /// The limit lives on [`KvClient`] rather than on [`ConnectOptions`], so every KV
    /// request has to apply it to its own client.
    pub fn kv_client(&self, conn: &Client) -> KvClient {
        let kv = conn.kv_client();
        match self.max_decoding_message_size {
            Some(size) => kv.max_decoding_message_size(size),
            None => kv,
        }
    }
}

impl EtcdCore {
    pub async fn get(&self, key: &str) -> Result<Option<Buffer>> {
        let client = self.conn().await?;
        let resp = self
            .kv_client(&client)
            .get(key, None)
            .await
            .map_err(format_etcd_error)?;
        if let Some(kv) = resp.kvs().first() {
            Ok(Some(Buffer::from(kv.value().to_vec())))
        } else {
            Ok(None)
        }
    }

    pub async fn set(&self, key: &str, value: Buffer) -> Result<()> {
        let mut client = self.conn().await?;
        let _ = client
            .put(key, value.to_vec(), None)
            .await
            .map_err(format_etcd_error)?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut client = self.conn().await?;
        let _ = client.delete(key, None).await.map_err(format_etcd_error)?;
        Ok(())
    }
}

use etcd_client::Error as EtcdError;

pub fn format_etcd_error(e: EtcdError) -> Error {
    Error::new(ErrorKind::Unexpected, e.to_string().as_str())
        .set_source(e)
        .set_temporary()
}
