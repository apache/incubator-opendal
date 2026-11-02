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
use std::time::Duration;

use asyncband::pool::ManageObject;
use asyncband::pool::ObjectStatus;
use asyncband::pool::bounded;
use opendal_core::raw::*;
use opendal_core::*;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;

use super::binary;

#[derive(Debug)]
pub enum SocketStream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
}

impl SocketStream {
    pub async fn connect_tcp(addr_str: &str) -> io::Result<Self> {
        let stream = TcpStream::connect(addr_str).await?;
        Ok(SocketStream::Tcp(stream))
    }

    #[cfg(unix)]
    pub async fn connect_unix(path: &str) -> io::Result<Self> {
        let stream = UnixStream::connect(path).await?;
        Ok(SocketStream::Unix(stream))
    }
}

impl AsyncRead for SocketStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            SocketStream::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            #[cfg(unix)]
            SocketStream::Unix(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for SocketStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            SocketStream::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            #[cfg(unix)]
            SocketStream::Unix(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            SocketStream::Tcp(s) => Pin::new(s).poll_flush(cx),
            #[cfg(unix)]
            SocketStream::Unix(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            SocketStream::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            #[cfg(unix)]
            SocketStream::Unix(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

/// Endpoint for memcached connection.
#[derive(Clone, Debug)]
pub enum Endpoint {
    Tcp(String), // host:port
    #[cfg(unix)]
    Unix(String), // socket path
}

/// A connection manager for `memcache_async::ascii::Protocol`.
#[derive(Clone)]
struct MemcacheConnectionManager {
    endpoint: Endpoint,
    username: Option<String>,
    password: Option<String>,
}

impl MemcacheConnectionManager {
    fn new(endpoint: Endpoint, username: Option<String>, password: Option<String>) -> Self {
        Self {
            endpoint,
            username,
            password,
        }
    }
}

impl ManageObject for MemcacheConnectionManager {
    type Object = binary::Connection;
    type Error = Error;

    async fn create(&self) -> Result<Self::Object, Self::Error> {
        let conn = match &self.endpoint {
            Endpoint::Tcp(addr) => SocketStream::connect_tcp(addr)
                .await
                .map_err(new_std_io_error)?,
            #[cfg(unix)]
            Endpoint::Unix(path) => SocketStream::connect_unix(path)
                .await
                .map_err(new_std_io_error)?,
        };

        let mut conn = binary::Connection::new(conn);

        if let (Some(username), Some(password)) = (self.username.as_ref(), self.password.as_ref()) {
            conn.auth(username, password).await?;
        }
        Ok(conn)
    }

    async fn is_recyclable(
        &self,
        o: &mut Self::Object,
        _: &ObjectStatus,
    ) -> Result<(), Self::Error> {
        match o.version().await {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MemcachedCore {
    default_ttl: Option<Duration>,
    conn: Arc<bounded::Pool<MemcacheConnectionManager>>,
}

/// memcached reads an expiration larger than 30 days as an absolute Unix
/// timestamp rather than as an offset from now, so a longer TTL sent verbatim
/// lands in 1970 and the server stores the item as already expired.
///
/// See the `Expiration times` section of memcached's protocol.txt. The builder
/// rejects a `default_ttl` above this, so `set` can pass the value through.
pub(super) const MAX_RELATIVE_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 30;

impl MemcachedCore {
    pub fn new(
        endpoint: Endpoint,
        username: Option<String>,
        password: Option<String>,
        default_ttl: Option<Duration>,
        connection_pool_max_size: Option<usize>,
    ) -> Self {
        let conn = bounded::Pool::new(
            bounded::PoolConfig::new(connection_pool_max_size.unwrap_or(10)),
            MemcacheConnectionManager::new(endpoint, username, password),
        );

        Self { default_ttl, conn }
    }

    async fn conn(&self) -> Result<bounded::Object<MemcacheConnectionManager>> {
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
        let result = conn.get(&percent_encode_path(key)).await?;
        Ok(result.map(Buffer::from))
    }

    pub async fn set(&self, key: &str, value: Buffer) -> Result<()> {
        let mut conn = self.conn().await?;

        conn.set(
            &percent_encode_path(key),
            &value.to_vec(),
            // The builder rejects anything above MAX_RELATIVE_EXPIRATION_SECS, so
            // this stays an offset and never lands in memcached's absolute range.
            self.default_ttl
                .map(|v| v.as_secs() as u32)
                .unwrap_or_default(),
        )
        .await
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.conn().await?;

        conn.delete(&percent_encode_path(key)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    // regression test for connecting to a webdav server.
    // Because setting up a dedicated behavior test is expensive, we choose to test `SocketStream::connect_tcp` instead.
    // In the future, we could set up a webdav server to test TCP connection properly.
    #[tokio::test]
    async fn connect_tcp_socket() -> std::io::Result<()> {
        for addr in &["127.0.0.1:11211", "localhost:11211", "[::1]:11211"] {
            let listener = TcpListener::bind(addr).await?;
            let addr = listener.local_addr()?.to_string();

            let accepted = tokio::spawn(async move {
                let _ = listener.accept().await?;
                Ok::<_, std::io::Error>(())
            });

            let _stream = SocketStream::connect_tcp(&addr).await?;
            accepted.await.unwrap()?;
        }

        Ok(())
    }

    // regression test for connecting to a webdav server.
    // Because setting up a dedicated behavior test is expensive, we choose to test `SocketStream::connect_unix` instead.
    // In the future, we could set up a webdav server to test UNIX socket connection properly.
    #[cfg(unix)]
    #[tokio::test]
    async fn connect_unix_socket() -> std::io::Result<()> {
        use std::time::{SystemTime, UNIX_EPOCH};
        use tokio::net::UnixListener;

        let path = std::env::temp_dir().join(format!(
            "opendal-memcached-{}.sock",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let listener = UnixListener::bind(&path)?;

        let accepted = tokio::spawn(async move {
            let _ = listener.accept().await?;
            Ok::<_, std::io::Error>(())
        });

        let _stream = SocketStream::connect_unix(path.to_str().unwrap()).await?;
        accepted.await.unwrap()?;

        let _ = std::fs::remove_file(path);
        Ok(())
    }
}
