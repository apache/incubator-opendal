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

use std::env;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::str::FromStr;

use futures::TryStreamExt;
use http::Request;
use http::Response;
use log::debug;
use reqwest::redirect::Policy;
use reqwest::ClientBuilder;
use reqwest::Url;

use super::AsyncBody;
use super::Body;
use crate::http_util::body::IncomingAsyncBody;
use crate::io_util::into_reader;

/// HttpClient that used across opendal.
#[derive(Clone)]
pub struct HttpClient {
    async_client: reqwest::Client,
    sync_client: ureq::Agent,
}

impl Default for HttpClient {
    fn default() -> Self {
        HttpClient::new()
    }
}

/// We don't want users to know details about our clients.
impl Debug for HttpClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient").finish()
    }
}

impl HttpClient {
    /// Create a new http client.
    pub fn new() -> Self {
        let async_client = {
            let mut builder = ClientBuilder::new();

            // Make sure we don't enable auto gzip decompress.
            builder = builder.no_gzip();
            // Make sure we don't enable auto brotli decompress.
            builder = builder.no_brotli();
            // Make sure we don't enable auto deflate decompress.
            builder = builder.no_deflate();
            // Redirect will be handled by ourselves.
            builder = builder.redirect(Policy::none());

            builder.build().expect("reqwest client must build succeed")
        };

        let sync_client = {
            let mut builder = ureq::AgentBuilder::new();

            for key in ["http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY"] {
                if let Ok(proxy) = env::var(key) {
                    // Ignore proxy setting if proxy is invalid.
                    if let Ok(proxy) = ureq::Proxy::new(proxy) {
                        debug!("sync client: set proxy to {proxy:?}");
                        builder = builder.proxy(proxy);
                    }
                }
            }

            builder.build()
        };

        HttpClient {
            async_client,
            sync_client,
        }
    }

    /// Send a request in blocking way.
    pub fn send(&self, req: Request<Body>) -> Result<Response<Body>> {
        let (parts, body) = req.into_parts();

        let mut ur = self
            .sync_client
            .request(parts.method.as_str(), &parts.uri.to_string());
        for (k, v) in parts.headers.iter() {
            ur = ur.set(k.as_str(), v.to_str().expect("must be valid header"));
        }

        let resp = match ur.send(body) {
            Ok(resp) => resp,
            Err(err_resp) => match err_resp {
                ureq::Error::Status(_code, resp) => resp,
                ureq::Error::Transport(transport) => {
                    let kind = match transport.kind() {
                        ureq::ErrorKind::Dns
                        | ureq::ErrorKind::ConnectionFailed
                        | ureq::ErrorKind::Io => ErrorKind::Interrupted,
                        _ => ErrorKind::Other,
                    };

                    return Err(Error::new(kind, transport));
                }
            },
        };

        let mut hr = Response::builder().status(resp.status());
        for name in resp.headers_names() {
            if let Some(value) = resp.header(&name) {
                hr = hr.header(name, value);
            }
        }

        let resp = hr
            .body(Body::Reader(Box::new(resp.into_reader())))
            .expect("response must build succeed");

        Ok(resp)
    }

    /// Send a request in async way.
    pub async fn send_async(&self, req: Request<AsyncBody>) -> Result<Response<IncomingAsyncBody>> {
        let (parts, body) = req.into_parts();

        let mut req_builder = self
            .async_client
            .request(
                parts.method,
                Url::from_str(&parts.uri.to_string()).expect("input request url must be valid"),
            )
            .version(parts.version)
            .headers(parts.headers);

        req_builder = if let AsyncBody::Multipart(field, r) = body {
            let mut form = reqwest::multipart::Form::new();
            let part = reqwest::multipart::Part::stream(AsyncBody::Reader(r));
            form = form.part(field, part);

            req_builder.multipart(form)
        } else {
            req_builder.body(body)
        };

        let resp = req_builder.send().await.map_err(|err| {
            let kind = error_kind_from_reqwest_error(&err);

            Error::new(kind, err)
        })?;

        let mut hr = Response::builder()
            .version(resp.version())
            .status(resp.status());
        for (k, v) in resp.headers().iter() {
            hr = hr.header(k, v);
        }

        let stream = resp.bytes_stream().map_err(|err| {
            let kind = error_kind_from_reqwest_error(&err);

            Error::new(kind, err)
        });
        let body = IncomingAsyncBody::new(Box::new(into_reader(stream)));

        let resp = hr.body(body).expect("response must build succeed");

        Ok(resp)
    }
}

fn error_kind_from_reqwest_error(err: &reqwest::Error) -> ErrorKind {
    // Builder related error should not be retried.
    if err.is_builder() {
        return ErrorKind::Other;
    }
    // Error returned by RedirectPolicy.
    //
    // We don't set this by hand, just don't allow retry.
    if err.is_redirect() {
        return ErrorKind::Other;
    }
    // We never use `Response::error_for_status`, just don't allow retry.
    //
    // Status should be checked by our services.
    if err.is_status() {
        return ErrorKind::Other;
    }

    ErrorKind::Interrupted
}
