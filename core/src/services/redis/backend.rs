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

use bb8::RunError;
use futures::Stream;
use futures::StreamExt;
use http::Uri;
use ouroboros::self_referencing;
use redis::cluster::ClusterClient;
use redis::cluster::ClusterClientBuilder;
use redis::AsyncIter;
use redis::Client;
use redis::ConnectionAddr;
use redis::ConnectionInfo;
use redis::ProtocolVersion;
use redis::RedisConnectionInfo;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::path::PathBuf;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use tokio::sync::OnceCell;

use crate::raw::adapters::kv;
use crate::raw::*;
use crate::services::RedisConfig;
use crate::*;

use super::core::*;
const DEFAULT_REDIS_ENDPOINT: &str = "tcp://127.0.0.1:6379";
const DEFAULT_REDIS_PORT: u16 = 6379;

impl Configurator for RedisConfig {
    type Builder = RedisBuilder;
    fn into_builder(self) -> Self::Builder {
        RedisBuilder { config: self }
    }
}

/// [Redis](https://redis.io/) services support.
#[doc = include_str!("docs.md")]
#[derive(Clone, Default)]
pub struct RedisBuilder {
    config: RedisConfig,
}

impl Debug for RedisBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("RedisBuilder");

        d.field("config", &self.config);
        d.finish_non_exhaustive()
    }
}

impl RedisBuilder {
    /// set the network address of redis service.
    ///
    /// currently supported schemes:
    /// - no scheme: will be seen as "tcp"
    /// - "tcp" or "redis": unsecured redis connections
    /// - "rediss": secured redis connections
    /// - "unix" or "redis+unix": unix socket connection
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            self.config.endpoint = Some(endpoint.to_owned());
        }
        self
    }

    /// set the network address of redis cluster service.
    /// This parameter is mutually exclusive with the endpoint parameter.
    ///
    /// currently supported schemes:
    /// - no scheme: will be seen as "tcp"
    /// - "tcp" or "redis": unsecured redis connections
    /// - "rediss": secured redis connections
    /// - "unix" or "redis+unix": unix socket connection
    pub fn cluster_endpoints(mut self, cluster_endpoints: &str) -> Self {
        if !cluster_endpoints.is_empty() {
            self.config.cluster_endpoints = Some(cluster_endpoints.to_owned());
        }
        self
    }

    /// set the username for redis
    ///
    /// default: no username
    pub fn username(mut self, username: &str) -> Self {
        if !username.is_empty() {
            self.config.username = Some(username.to_owned());
        }
        self
    }

    /// set the password for redis
    ///
    /// default: no password
    pub fn password(mut self, password: &str) -> Self {
        if !password.is_empty() {
            self.config.password = Some(password.to_owned());
        }
        self
    }

    /// set the db used in redis
    ///
    /// default: 0
    pub fn db(mut self, db: i64) -> Self {
        self.config.db = db;
        self
    }

    /// Set the default ttl for redis services.
    ///
    /// If set, we will specify `EX` for write operations.
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.config.default_ttl = Some(ttl);
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
}

impl Builder for RedisBuilder {
    const SCHEME: Scheme = Scheme::Redis;
    type Config = RedisConfig;

    fn build(self) -> Result<impl Access> {
        let root = normalize_root(
            self.config
                .root
                .clone()
                .unwrap_or_else(|| "/".to_string())
                .as_str(),
        );

        if let Some(endpoints) = self.config.cluster_endpoints.clone() {
            let mut cluster_endpoints: Vec<ConnectionInfo> = Vec::default();
            for endpoint in endpoints.split(',') {
                cluster_endpoints.push(self.get_connection_info(endpoint.to_string())?);
            }
            let mut client_builder = ClusterClientBuilder::new(cluster_endpoints);
            if let Some(username) = &self.config.username {
                client_builder = client_builder.username(username.clone());
            }
            if let Some(password) = &self.config.password {
                client_builder = client_builder.password(password.clone());
            }
            let client = client_builder.build().map_err(format_redis_error)?;

            let conn = OnceCell::new();

            Ok(RedisBackend::new(Adapter {
                addr: endpoints,
                client: None,
                cluster_client: Some(client),
                conn,
                default_ttl: self.config.default_ttl,
            })
            .with_normalized_root(root))
        } else {
            let endpoint = self
                .config
                .endpoint
                .clone()
                .unwrap_or_else(|| DEFAULT_REDIS_ENDPOINT.to_string());

            let client =
                Client::open(self.get_connection_info(endpoint.clone())?).map_err(|e| {
                    Error::new(ErrorKind::ConfigInvalid, "invalid or unsupported scheme")
                        .with_context("service", Scheme::Redis)
                        .with_context("endpoint", self.config.endpoint.as_ref().unwrap())
                        .with_context("db", self.config.db.to_string())
                        .set_source(e)
                })?;

            let conn = OnceCell::new();
            Ok(RedisBackend::new(Adapter {
                addr: endpoint,
                client: Some(client),
                cluster_client: None,
                conn,
                default_ttl: self.config.default_ttl,
            })
            .with_normalized_root(root))
        }
    }
}

impl RedisBuilder {
    fn get_connection_info(&self, endpoint: String) -> Result<ConnectionInfo> {
        let ep_url = endpoint.parse::<Uri>().map_err(|e| {
            Error::new(ErrorKind::ConfigInvalid, "endpoint is invalid")
                .with_context("service", Scheme::Redis)
                .with_context("endpoint", endpoint)
                .set_source(e)
        })?;

        let con_addr = match ep_url.scheme_str() {
            Some("tcp") | Some("redis") | None => {
                let host = ep_url
                    .host()
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| "127.0.0.1".to_string());
                let port = ep_url.port_u16().unwrap_or(DEFAULT_REDIS_PORT);
                ConnectionAddr::Tcp(host, port)
            }
            Some("rediss") => {
                let host = ep_url
                    .host()
                    .map(|h| h.to_string())
                    .unwrap_or_else(|| "127.0.0.1".to_string());
                let port = ep_url.port_u16().unwrap_or(DEFAULT_REDIS_PORT);
                ConnectionAddr::TcpTls {
                    host,
                    port,
                    insecure: false,
                    tls_params: None,
                }
            }
            Some("unix") | Some("redis+unix") => {
                let path = PathBuf::from(ep_url.path());
                ConnectionAddr::Unix(path)
            }
            Some(s) => {
                return Err(
                    Error::new(ErrorKind::ConfigInvalid, "invalid or unsupported scheme")
                        .with_context("service", Scheme::Redis)
                        .with_context("scheme", s),
                )
            }
        };

        let redis_info = RedisConnectionInfo {
            db: self.config.db,
            username: self.config.username.clone(),
            password: self.config.password.clone(),
            protocol: ProtocolVersion::RESP2,
        };

        Ok(ConnectionInfo {
            addr: con_addr,
            redis: redis_info,
        })
    }
}

/// Backend for redis services.
pub type RedisBackend = kv::Backend<Adapter>;

#[derive(Clone)]
pub struct Adapter {
    addr: String,
    client: Option<Client>,
    cluster_client: Option<ClusterClient>,
    conn: OnceCell<bb8::Pool<RedisConnectionManager>>,

    default_ttl: Option<Duration>,
}

// implement `Debug` manually, or password may be leaked.
impl Debug for Adapter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("Adapter");

        ds.field("addr", &self.addr);
        ds.finish()
    }
}

impl Adapter {
    async fn conn(&self) -> Result<bb8::PooledConnection<'_, RedisConnectionManager>> {
        let pool = self.pool().await?;
        Adapter::conn_from_pool(pool).await
    }

    async fn conn_from_pool(
        pool: &bb8::Pool<RedisConnectionManager>,
    ) -> Result<bb8::PooledConnection<RedisConnectionManager>> {
        pool.get().await.map_err(|err| match err {
            RunError::TimedOut => {
                Error::new(ErrorKind::Unexpected, "get connection from pool failed").set_temporary()
            }
            RunError::User(err) => err,
        })
    }

    async fn pool(&self) -> Result<&bb8::Pool<RedisConnectionManager>> {
        self.conn
            .get_or_try_init(|| async {
                bb8::Pool::builder()
                    .build(self.get_redis_connection_manager())
                    .await
                    .map_err(|err| {
                        Error::new(ErrorKind::ConfigInvalid, "connect to redis failed")
                            .set_source(err)
                    })
            })
            .await
    }

    fn get_redis_connection_manager(&self) -> RedisConnectionManager {
        if let Some(_client) = self.client.clone() {
            RedisConnectionManager {
                client: self.client.clone(),
                cluster_client: None,
            }
        } else {
            RedisConnectionManager {
                client: None,
                cluster_client: self.cluster_client.clone(),
            }
        }
    }
}

#[self_referencing]
struct RedisAsyncConnIter<'a> {
    conn: bb8::PooledConnection<'a, RedisConnectionManager>,

    #[borrows(mut conn)]
    #[not_covariant]
    iter: AsyncIter<'this, String>,
}

#[self_referencing]
pub struct RedisScanner {
    pool: bb8::Pool<RedisConnectionManager>,
    path: String,

    #[borrows(pool, path)]
    #[not_covariant]
    inner: RedisAsyncConnIter<'this>,
}

unsafe impl Sync for RedisScanner {}

impl Stream for RedisScanner {
    type Item = Result<String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.with_inner_mut(|s| s.with_iter_mut(|v| v.poll_next_unpin(cx).map(|v| v.map(Ok))))
    }
}

impl kv::Scan for RedisScanner {
    async fn next(&mut self) -> Result<Option<String>> {
        <Self as StreamExt>::next(self).await.transpose()
    }
}

impl kv::Adapter for Adapter {
    type Scanner = RedisScanner;

    fn info(&self) -> kv::Info {
        kv::Info::new(
            Scheme::Redis,
            self.addr.as_str(),
            Capability {
                read: true,
                write: true,
                // due to limitation of Redis itself,
                // on cluster mode we cannot get full list of keys via SCAN,
                // so here we disable it on cluster mode to avoid confusions.
                // TODO: we can perform multiple SCAN on each cluster node
                // and merge the result to simulate the behavior of list here.
                list: self.cluster_client.is_none(),

                ..Default::default()
            },
        )
    }

    async fn get(&self, key: &str) -> Result<Option<Buffer>> {
        let mut conn = self.conn().await?;
        let result = conn.get(key).await?;
        Ok(result)
    }

    async fn set(&self, key: &str, value: Buffer) -> Result<()> {
        let mut conn = self.conn().await?;
        let value = value.to_vec();
        conn.set(key, value, self.default_ttl).await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.conn().await?;
        conn.delete(key).await?;
        Ok(())
    }

    async fn append(&self, key: &str, value: &[u8]) -> Result<()> {
        let mut conn = self.conn().await?;
        conn.append(key, value).await?;
        Ok(())
    }

    async fn scan(&self, path: &str) -> Result<Self::Scanner> {
        let pool = self.pool().await?.clone();

        RedisScanner::try_new_async_send(pool, path.to_string(), |pool, path| {
            Box::pin(async {
                let conn = Adapter::conn_from_pool(pool).await?;
                RedisAsyncConnIter::try_new_async_send(conn, |conn| {
                    Box::pin(async { conn.scan(path).await })
                })
                .await
            })
        })
        .await
    }
}
