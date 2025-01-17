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

use std::time::Duration;

use bb8::RunError;
use tokio::net::TcpStream;
use tokio::sync::OnceCell;
use tokio_native_tls::native_tls;
use tokio_native_tls::TlsConnector;
use tokio_native_tls::TlsStream;

use super::binary;
use crate::raw::adapters::kv;
use crate::raw::*;
use crate::services::MemcachedConfig;
use crate::*;

impl Configurator for MemcachedConfig {
    type Builder = MemcachedBuilder;
    fn into_builder(self) -> Self::Builder {
        MemcachedBuilder { config: self }
    }
}

/// [Memcached](https://memcached.org/) service support.
#[doc = include_str!("docs.md")]
#[derive(Clone, Default)]
pub struct MemcachedBuilder {
    config: MemcachedConfig,
}

impl MemcachedBuilder {
    /// set the network address of memcached service.
    ///
    /// For example: "tcp://localhost:11211"
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            self.config.endpoint = Some(endpoint.to_owned());
        }
        self
    }

    /// set the working directory, all operations will be performed under it.
    ///
    /// default: "/"
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// set the username.
    pub fn username(mut self, username: &str) -> Self {
        self.config.username = Some(username.to_string());
        self
    }

    /// set the password.
    pub fn password(mut self, password: &str) -> Self {
        self.config.password = Some(password.to_string());
        self
    }

    /// Set the default ttl for memcached services.
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.config.default_ttl = Some(ttl);
        self
    }

    /// Enable TLS for the connection.
    ///
    /// Required for AWS ElastiCache Memcached serverless instances.
    pub fn enable_tls(mut self, enable: bool) -> Self {
        self.config.enable_tls = Some(enable);
        self
    }

    /// Set the CA certificate file path for TLS verification.
    pub fn ca_cert(mut self, ca_cert: &str) -> Self {
        if !ca_cert.is_empty() {
            self.config.ca_cert = Some(ca_cert.to_string());
        }
        self
    }
}

impl Builder for MemcachedBuilder {
    const SCHEME: Scheme = Scheme::Memcached;
    type Config = MemcachedConfig;

    fn build(self) -> Result<impl Access> {
        let endpoint = self.config.endpoint.clone().ok_or_else(|| {
            Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_context("service", Scheme::Memcached)
        })?;
        let uri = http::Uri::try_from(&endpoint).map_err(|err| {
            Error::new(ErrorKind::ConfigInvalid, "endpoint is invalid")
                .with_context("service", Scheme::Memcached)
                .with_context("endpoint", &endpoint)
                .set_source(err)
        })?;

        let authority = uri.authority().ok_or_else(|| {
            Error::new(ErrorKind::ConfigInvalid, "endpoint must contain authority")
                .with_context("service", Scheme::Memcached)
                .with_context("endpoint", &endpoint)
        })?;

        let root = normalize_root(&self.config.root.unwrap_or_default())?;

        let manager = MemcacheConnectionManager::new(
            authority.as_str(),
            self.config.username,
            self.config.password,
            self.config.enable_tls.unwrap_or(false),
            self.config.ca_cert,
        );

        let pool = bb8::Pool::builder()
            .max_size(1)
            .build(manager)
            .map_err(new_connection_error)?;

        Ok(MemcachedBackend::new(Adapter {
            pool: pool.into(),
            root,
            default_ttl: self.config.default_ttl,
        }))
    }
}

/// Backend for memcached services.
pub type MemcachedBackend = kv::Backend<Adapter>;

#[derive(Clone, Debug)]
pub struct Adapter {
    pool: bb8::Pool<MemcacheConnectionManager>,
    root: String,
    default_ttl: Option<Duration>,
}

impl Adapter {
    async fn conn(&self) -> Result<bb8::PooledConnection<'_, MemcacheConnectionManager>> {
        self.pool.get().await.map_err(|err| match err {
            RunError::TimedOut => {
                Error::new(ErrorKind::Unexpected, "get connection from pool failed").set_temporary()
            }
            RunError::User(err) => err,
        })
    }
}

impl kv::Adapter for Adapter {
    type Scanner = ();

    fn info(&self) -> kv::Info {
        kv::Info::new(
            Scheme::Memcached,
            "memcached",
            Capability {
                read: true,
                write: true,
                shared: true,

                ..Default::default()
            },
        )
    }

    async fn get(&self, key: &str) -> Result<Option<Buffer>> {
        let mut conn = self.conn().await?;
        let result = conn.get(&percent_encode_path(key)).await?;
        Ok(result.map(Buffer::from))
    }

    async fn set(&self, key: &str, value: Buffer) -> Result<()> {
        let mut conn = self.conn().await?;

        conn.set(
            &percent_encode_path(key),
            &value.to_vec(),
            // Set expiration to 0 if ttl not set.
            self.default_ttl
                .map(|v| v.as_secs() as u32)
                .unwrap_or_default(),
        )
        .await
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.conn().await?;

        conn.delete(&percent_encode_path(key)).await
    }
}

/// A `bb8::ManageConnection` for `memcache_async::ascii::Protocol`.
#[derive(Clone, Debug)]
pub struct MemcacheConnectionManager {
    address: String,
    username: Option<String>,
    password: Option<String>,
    enable_tls: bool,
    ca_cert: Option<String>,
}

impl MemcacheConnectionManager {
    pub fn new(
        address: &str,
        username: Option<String>,
        password: Option<String>,
        enable_tls: bool,
        ca_cert: Option<String>,
    ) -> Self {
        Self {
            address: address.to_string(),
            username,
            password,
            enable_tls,
            ca_cert,
        }
    }
}

#[async_trait::async_trait]
impl bb8::ManageConnection for MemcacheConnectionManager {
    type Connection = binary::Connection;
    type Error = Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let stream = if self.enable_tls {
            let mut builder = native_tls::TlsConnector::builder();
            
            // If CA cert is provided, add it to the builder
            if let Some(ca_cert) = &self.ca_cert {
                builder.add_root_certificate(
                    native_tls::Certificate::from_pem(
                        &std::fs::read(ca_cert).map_err(|err| {
                            Error::new(ErrorKind::ConfigInvalid, "failed to read CA certificate")
                                .set_source(err)
                        })?
                    ).map_err(|err| {
                        Error::new(ErrorKind::ConfigInvalid, "invalid CA certificate")
                            .set_source(err)
                    })?
                );
            }

            let connector = builder.build().map_err(|err| {
                Error::new(ErrorKind::ConfigInvalid, "failed to build TLS connector")
                    .set_source(err)
            })?;
            let connector = TlsConnector::from(connector);

            let tcp = TcpStream::connect(&self.address).await.map_err(|err| {
                Error::new(ErrorKind::ConfigInvalid, "failed to connect")
                    .set_source(err)
            })?;

            let domain = self.address.split(':').next().unwrap_or(&self.address);
            let tls_stream = connector.connect(domain, tcp).await.map_err(|err| {
                Error::new(ErrorKind::ConfigInvalid, "TLS handshake failed")
                    .set_source(err)
            })?;

            binary::Connection::new(Box::new(tls_stream)).await
        } else {
            let tcp = TcpStream::connect(&self.address).await.map_err(|err| {
                Error::new(ErrorKind::ConfigInvalid, "failed to connect")
                    .set_source(err)
            })?;
            binary::Connection::new(Box::new(tcp)).await
        };

        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            stream.authenticate(username, password).await?;
        }

        Ok(stream)
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.version().await.map(|_| ())
    }

    fn has_broken(&self, _: &mut Self::Connection) -> bool {
        false
    }
}
