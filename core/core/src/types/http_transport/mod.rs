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
use std::fmt::Formatter;
use std::future::Future;
use std::sync::Arc;
use std::sync::OnceLock;

#[cfg(feature = "reqsign")]
use bytes::Bytes;
use http::Request;
use http::Response;

use crate::raw::BoxedFuture;
use crate::raw::oio::ReadStream;
use crate::types::Buffer;
use crate::types::Error;
use crate::types::ErrorKind;
use crate::types::Result;

mod body;
pub use body::HttpBody;

mod uri;
pub use uri::{HttpRedirect, HttpUri};

static DEFAULT_HTTP_TRANSPORTER: OnceLock<HttpTransporter> = OnceLock::new();

/// HTTP transport used by OpenDAL services.
///
/// Implement this trait to provide a custom HTTP backend. A transport must
/// support 3xx redirection.
/// It can support [`HttpRedirect`] to let services reuse redirect destinations.
pub trait HttpTransport: Send + Sync + Unpin + 'static {
    /// Fetch a request and return a streamable [`HttpBody`].
    #[cfg(not(target_arch = "wasm32"))]
    fn fetch(
        &self,
        req: Request<Buffer>,
    ) -> impl Future<Output = Result<Response<HttpBody>>> + Send;

    /// Fetch a request and return a streamable [`HttpBody`].
    #[cfg(target_arch = "wasm32")]
    fn fetch(&self, req: Request<Buffer>) -> impl Future<Output = Result<Response<HttpBody>>>;
}

/// Object-safe version of [`HttpTransport`].
pub(crate) trait HttpTransportDyn: Send + Sync + Unpin + 'static {
    /// Fetch a request through a boxed future.
    fn fetch_dyn(&self, req: Request<Buffer>) -> BoxedFuture<'_, Result<Response<HttpBody>>>;
}

impl<T: HttpTransport + ?Sized> HttpTransportDyn for T {
    fn fetch_dyn(&self, req: Request<Buffer>) -> BoxedFuture<'_, Result<Response<HttpBody>>> {
        Box::pin(self.fetch(req))
    }
}

/// Type-erased HTTP transport handle.
#[derive(Clone)]
pub struct HttpTransporter {
    inner: Arc<dyn HttpTransportDyn>,
}

impl Debug for HttpTransporter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpTransporter").finish()
    }
}

impl Default for HttpTransporter {
    fn default() -> Self {
        Self::new(DefaultHttpTransport)
    }
}

impl HttpTransporter {
    /// Create a new `HttpTransporter` from an [`HttpTransport`].
    pub fn new(transport: impl HttpTransport) -> Self {
        Self {
            inner: Arc::new(transport),
        }
    }

    /// Install the process-wide default HTTP transport.
    ///
    /// The first installed transport wins. Later calls are ignored.
    ///
    /// The `opendal` facade calls this before `main` when
    /// `auto-register-services` and a default HTTP transport feature are enabled.
    /// Applications that need to install their own process-wide default should
    /// disable `auto-register-services`. This does not affect transports
    /// configured directly on an operator.
    pub fn install_default(transport: impl HttpTransport) {
        let _ = DEFAULT_HTTP_TRANSPORTER.set(Self::new(transport));
    }

    /// Send a request and consume the response body into a [`Buffer`].
    pub async fn send(&self, req: Request<Buffer>) -> Result<Response<Buffer>> {
        let (parts, mut body) = self.fetch(req).await?.into_parts();
        let buffer = body.read_all().await?;
        Ok(Response::from_parts(parts, buffer))
    }

    /// Fetch a request and return a streamable [`HttpBody`].
    ///
    /// Mark credential headers as sensitive for diagnostics without changing
    /// their values.
    pub async fn fetch(&self, mut req: Request<Buffer>) -> Result<Response<HttpBody>> {
        // Mark credentials at the shared boundary before any transport wrapper
        // formats the request. Values on the wire remain unchanged.
        for (name, value) in req.headers_mut() {
            if matches!(
                name.as_str(),
                "authorization" | "proxy-authorization" | "cookie" | "cookie2"
            ) {
                value.set_sensitive(true);
            }
        }
        let mut resp = self.inner.fetch_dyn(req).await?;
        for (name, value) in resp.headers_mut() {
            if matches!(
                name.as_str(),
                "set-cookie" | "www-authenticate" | "proxy-authenticate"
            ) {
                value.set_sensitive(true);
            }
        }
        let (mut parts, body) = resp.into_parts();
        HttpUri::from_response_location(&mut parts);
        Ok(Response::from_parts(parts, body))
    }
}

impl HttpTransport for HttpTransporter {
    #[cfg(not(target_arch = "wasm32"))]
    async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
        HttpTransporter::fetch(self, req).await
    }

    #[cfg(target_arch = "wasm32")]
    async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
        HttpTransporter::fetch(self, req).await
    }
}

#[cfg(feature = "reqsign")]
impl reqsign_core::HttpSend for HttpTransporter {
    async fn http_send(&self, req: Request<Bytes>) -> reqsign_core::Result<Response<Bytes>> {
        let mut req = req.map(Buffer::from);
        // reqsign parses credential responses (tokens, metadata) as plain
        // bytes and never looks at `Content-Encoding`. A user transport may
        // send `Accept-Encoding: gzip` by default without decoding the reply,
        // so ask for an uncompressed body unless reqsign chose an encoding.
        if !req.headers().contains_key(http::header::ACCEPT_ENCODING) {
            req.headers_mut().insert(
                http::header::ACCEPT_ENCODING,
                http::HeaderValue::from_static("identity"),
            );
        }
        let resp = self.send(req).await.map_err(|err| {
            let retryable = err.is_temporary();
            reqsign_core::Error::unexpected("send request via OpenDAL HttpTransporter")
                .with_source(err)
                .set_retryable(retryable)
        })?;

        let (parts, body) = resp.into_parts();
        Ok(Response::from_parts(parts, body.to_bytes()))
    }
}

#[derive(Debug)]
struct DefaultHttpTransport;

impl HttpTransport for DefaultHttpTransport {
    #[cfg(not(target_arch = "wasm32"))]
    async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
        fetch_with_default_transport(req).await
    }

    #[cfg(target_arch = "wasm32")]
    async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
        fetch_with_default_transport(req).await
    }
}

async fn fetch_with_default_transport(req: Request<Buffer>) -> Result<Response<HttpBody>> {
    match DEFAULT_HTTP_TRANSPORTER.get() {
        Some(transport) => transport.fetch(req).await,
        None => Err(Error::new(
            ErrorKind::ConfigInvalid,
            "default HTTP transport is not installed",
        )
        .with_operation("HttpTransporter::default")
        .with_context(
            "advice",
            "call HttpTransporter::install_default before using HTTP services",
        )),
    }
}

#[cfg(all(test, feature = "reqsign", not(target_arch = "wasm32")))]
mod tests {
    use std::sync::Mutex;

    use http::header::ACCEPT_ENCODING;
    use reqsign_core::HttpSend;

    use super::*;

    /// Records the `Accept-Encoding` header of each request it receives.
    #[derive(Clone, Default)]
    struct RecordingTransport {
        accept_encoding: Arc<Mutex<Vec<Option<String>>>>,
    }

    impl HttpTransport for RecordingTransport {
        async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
            let value = req
                .headers()
                .get(ACCEPT_ENCODING)
                .map(|v| v.to_str().unwrap().to_string());
            self.accept_encoding.lock().unwrap().push(value);

            let body = Buffer::from("{}");
            let size = body.len() as u64;
            let stream = futures::stream::iter(vec![Ok(body)]);
            Ok(Response::new(HttpBody::new(stream, Some(size))))
        }
    }

    #[tokio::test]
    async fn test_http_send_requests_identity_encoding() {
        let transport = RecordingTransport::default();
        let transporter = HttpTransporter::new(transport.clone());

        let req = Request::get("http://127.0.0.1/token")
            .body(Bytes::new())
            .unwrap();
        let resp = transporter.http_send(req).await.unwrap();
        assert_eq!(resp.body().as_ref(), b"{}");

        assert_eq!(
            *transport.accept_encoding.lock().unwrap(),
            vec![Some("identity".to_string())]
        );
    }

    #[tokio::test]
    async fn test_http_send_keeps_explicit_accept_encoding() {
        let transport = RecordingTransport::default();
        let transporter = HttpTransporter::new(transport.clone());

        let req = Request::get("http://127.0.0.1/token")
            .header(ACCEPT_ENCODING, "br")
            .body(Bytes::new())
            .unwrap();
        transporter.http_send(req).await.unwrap();

        assert_eq!(
            *transport.accept_encoding.lock().unwrap(),
            vec![Some("br".to_string())]
        );
    }
}
