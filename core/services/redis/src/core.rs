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
use std::sync::Arc;

use asyncband::pool::{ManageObject, ObjectStatus, bounded};
use bytes::Bytes;
use redis::AsyncCommands;
use redis::Client;
use redis::Cmd;
use redis::Pipeline;
use redis::RedisError;
use redis::RedisFuture;
use redis::Value;
use redis::aio::ConnectionLike;
use redis::aio::ConnectionManager;
use redis::cluster::ClusterClient;
use redis::cluster_async::ClusterConnection;

use opendal_core::raw::*;
use opendal_core::*;

#[derive(Clone)]
pub enum RedisConnection {
    Normal(ConnectionManager),
    Cluster(ClusterConnection),
}

impl ConnectionLike for RedisConnection {
    fn req_packed_command<'a>(&'a mut self, cmd: &'a Cmd) -> RedisFuture<'a, Value> {
        match self {
            RedisConnection::Normal(conn) => conn.req_packed_command(cmd),
            RedisConnection::Cluster(conn) => conn.req_packed_command(cmd),
        }
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        cmd: &'a Pipeline,
        offset: usize,
        count: usize,
    ) -> RedisFuture<'a, Vec<Value>> {
        match self {
            RedisConnection::Normal(conn) => conn.req_packed_commands(cmd, offset, count),
            RedisConnection::Cluster(conn) => conn.req_packed_commands(cmd, offset, count),
        }
    }

    fn get_db(&self) -> i64 {
        match self {
            RedisConnection::Normal(conn) => conn.get_db(),
            RedisConnection::Cluster(conn) => conn.get_db(),
        }
    }
}

#[derive(Clone)]
pub struct RedisConnectionManager {
    client: Option<Client>,
    cluster_client: Option<ClusterClient>,
}

impl ManageObject for RedisConnectionManager {
    type Object = RedisConnection;
    type Error = Error;

    async fn create(&self) -> Result<RedisConnection, Self::Error> {
        if let Some(client) = self.client.clone() {
            ConnectionManager::new(client.clone())
                .await
                .map_err(format_redis_error)
                .map(RedisConnection::Normal)
        } else {
            self.cluster_client
                .clone()
                .unwrap()
                .get_async_connection()
                .await
                .map_err(format_redis_error)
                .map(RedisConnection::Cluster)
        }
    }

    async fn is_recyclable(
        &self,
        o: &mut Self::Object,
        _: &ObjectStatus,
    ) -> Result<(), Self::Error> {
        match o.ping::<String>().await {
            Ok(ref pong) => match pong.as_bytes() {
                b"PONG" => Ok(()),
                _ => Err(Error::new(ErrorKind::Unexpected, "PING ERROR")),
            },
            Err(err) => Err(format_redis_error(err)),
        }
    }
}

/// RedisCore holds the Redis connection and configuration
#[derive(Clone)]
pub struct RedisCore {
    addr: String,
    conn: Arc<bounded::Pool<RedisConnectionManager>>,
    default_ttl: Option<Duration>,
}

impl Debug for RedisCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisCore")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

impl RedisCore {
    pub fn new(
        endpoint: String,
        client: Option<Client>,
        cluster_client: Option<ClusterClient>,
        default_ttl: Option<Duration>,
        connection_pool_max_size: Option<usize>,
    ) -> Self {
        let manager = RedisConnectionManager {
            client,
            cluster_client,
        };
        let pool = bounded::Pool::new(
            bounded::PoolConfig::new(connection_pool_max_size.unwrap_or(10)),
            manager,
        );

        Self {
            addr: endpoint,
            conn: pool,
            default_ttl,
        }
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    pub async fn conn(&self) -> Result<bounded::Object<RedisConnectionManager>> {
        let fut = self.conn.get();

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

    pub async fn get(&self, key: &str) -> Result<Option<Buffer>> {
        let mut conn = self.conn().await?;
        let result: Option<Bytes> = conn.get(key).await.map_err(format_redis_error)?;
        Ok(result.map(Buffer::from))
    }

    pub async fn len(&self, key: &str) -> Result<Option<usize>> {
        let mut conn = self.conn().await?;
        let exists: bool = conn.exists(key).await.map_err(format_redis_error)?;
        if !exists {
            return Ok(None);
        }

        let len: usize = redis::cmd("STRLEN")
            .arg(key)
            .query_async(&mut *conn)
            .await
            .map_err(format_redis_error)?;
        Ok(Some(len))
    }

    pub async fn get_range(&self, key: &str, start: isize, end: isize) -> Result<Option<Buffer>> {
        let mut conn = self.conn().await?;
        let result: Option<Bytes> = conn
            .getrange(key, start, end)
            .await
            .map_err(format_redis_error)?;
        Ok(result.map(Buffer::from))
    }

    pub async fn set(&self, key: &str, value: Buffer) -> Result<()> {
        let mut conn = self.conn().await?;
        let value = value.to_vec();
        if let Some(dur) = self.default_ttl {
            let _: () = conn
                .set_ex(key, value, dur.as_secs())
                .await
                .map_err(format_redis_error)?;
        } else {
            let _: () = conn.set(key, value).await.map_err(format_redis_error)?;
        }
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.conn().await?;
        let _: () = conn.del(key).await.map_err(format_redis_error)?;
        Ok(())
    }
}

pub fn format_redis_error(e: RedisError) -> Error {
    Error::new(ErrorKind::Unexpected, e.category())
        .set_source(e)
        .set_temporary()
}
