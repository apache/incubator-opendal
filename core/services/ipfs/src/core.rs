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

use http::Request;
use http::Response;
use http::StatusCode;

use opendal_core::raw::*;
use opendal_core::*;

pub struct IpfsCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub endpoint: String,
    pub root: String,
}

impl Debug for IpfsCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpfsCore")
            .field("endpoint", &self.endpoint)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl IpfsCore {
    pub async fn ipfs_get(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let p = build_rooted_abs_path(&self.root, path);

        let url = format!("{}{}", self.endpoint, percent_encode_path(&p));

        let mut req = Request::get(&url);

        if !range.is_full() {
            req = req.header(http::header::RANGE, range.to_header());
        }

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("Get"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub async fn ipfs_head(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let p = build_rooted_abs_path(&self.root, path);

        let url = format!("{}{}", self.endpoint, percent_encode_path(&p));

        let req = Request::head(&url)
            .extension(Operation::Stat)
            .extension(ServiceOperation("Head"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn ipfs_list(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let p = build_rooted_abs_path(&self.root, path);

        let url = format!("{}{}", self.endpoint, percent_encode_path(&p));

        let mut req = Request::get(&url);

        // Use "application/vnd.ipld.raw" to disable IPLD codec deserialization
        // OpenDAL will parse ipld data directly.
        //
        // ref: https://github.com/ipfs/specs/blob/main/http-gateways/PATH_GATEWAY.md
        req = req.header(http::header::ACCEPT, "application/vnd.ipld.raw");

        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("List"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    /// IPFS's stat behavior highly depends on its implementation.
    ///
    /// Based on IPFS [Path Gateway Specification](https://github.com/ipfs/specs/blob/main/http-gateways/PATH_GATEWAY.md),
    /// response payload could be:
    ///
    /// > - UnixFS (implicit default)
    /// >   - File
    /// >     - Bytes representing file contents
    /// >   - Directory
    /// >     - Generated HTML with directory index
    /// >     - When `index.html` is present, gateway can skip generating directory index and return it instead
    /// > - Raw block (not this case)
    /// > - CAR (not this case)
    ///
    /// When we HEAD a given path, we could have the following responses:
    ///
    /// - File
    ///
    /// ```http
    /// :) curl -I https://ipfs.io/ipfs/QmPpCt1aYGb9JWJRmXRUnmJtVgeFFTJGzWFYEEX7bo9zGJ/normal_file
    /// HTTP/1.1 200 Connection established
    ///
    /// HTTP/2 200
    /// server: openresty
    /// date: Thu, 08 Sep 2022 00:48:50 GMT
    /// content-type: application/octet-stream
    /// content-length: 262144
    /// access-control-allow-methods: GET
    /// cache-control: public, max-age=29030400, immutable
    /// etag: "QmdP6teFTLSNVhT4W5jkhEuUBsjQ3xkp1GmRvDU6937Me1"
    /// x-ipfs-gateway-host: ipfs-bank11-fr2
    /// x-ipfs-path: /ipfs/QmPpCt1aYGb9JWJRmXRUnmJtVgeFFTJGzWFYEEX7bo9zGJ/normal_file
    /// x-ipfs-roots: QmPpCt1aYGb9JWJRmXRUnmJtVgeFFTJGzWFYEEX7bo9zGJ,QmdP6teFTLSNVhT4W5jkhEuUBsjQ3xkp1GmRvDU6937Me1
    /// x-ipfs-pop: ipfs-bank11-fr2
    /// timing-allow-origin: *
    /// x-ipfs-datasize: 262144
    /// access-control-allow-origin: *
    /// access-control-allow-methods: GET, POST, OPTIONS
    /// access-control-allow-headers: X-Requested-With, Range, Content-Range, X-Chunked-Output, X-Stream-Output
    /// access-control-expose-headers: Content-Range, X-Chunked-Output, X-Stream-Output
    /// x-ipfs-lb-pop: gateway-bank1-fr2
    /// strict-transport-security: max-age=31536000; includeSubDomains; preload
    /// x-proxy-cache: MISS
    /// accept-ranges: bytes
    /// ```
    ///
    /// - Dir with generated index
    ///
    /// ```http
    /// :( curl -I https://ipfs.io/ipfs/QmPpCt1aYGb9JWJRmXRUnmJtVgeFFTJGzWFYEEX7bo9zGJ/normal_dir
    /// HTTP/1.1 200 Connection established
    ///
    /// HTTP/2 200
    /// server: openresty
    /// date: Wed, 07 Sep 2022 08:46:13 GMT
    /// content-type: text/html
    /// vary: Accept-Encoding
    /// access-control-allow-methods: GET
    /// etag: "DirIndex-2b567f6r5vvdg_CID-QmY44DyCDymRN1Qy7sGbupz1ysMkXTWomAQku5vBg7fRQW"
    /// x-ipfs-gateway-host: ipfs-bank6-sg1
    /// x-ipfs-path: /ipfs/QmPpCt1aYGb9JWJRmXRUnmJtVgeFFTJGzWFYEEX7bo9zGJ/normal_dir
    /// x-ipfs-roots: QmPpCt1aYGb9JWJRmXRUnmJtVgeFFTJGzWFYEEX7bo9zGJ,QmY44DyCDymRN1Qy7sGbupz1ysMkXTWomAQku5vBg7fRQW
    /// x-ipfs-pop: ipfs-bank6-sg1
    /// timing-allow-origin: *
    /// access-control-allow-origin: *
    /// access-control-allow-methods: GET, POST, OPTIONS
    /// access-control-allow-headers: X-Requested-With, Range, Content-Range, X-Chunked-Output, X-Stream-Output
    /// access-control-expose-headers: Content-Range, X-Chunked-Output, X-Stream-Output
    /// x-ipfs-lb-pop: gateway-bank3-sg1
    /// strict-transport-security: max-age=31536000; includeSubDomains; preload
    /// x-proxy-cache: MISS
    /// ```
    ///
    /// - Dir with index.html
    ///
    /// ```http
    /// :) curl -I http://127.0.0.1:8080/ipfs/QmVturFGV3z4WsP7cRV8Ci4avCdGWYXk2qBKvtAwFUp5Az
    /// HTTP/1.1 302 Found
    /// Access-Control-Allow-Headers: Content-Type
    /// Access-Control-Allow-Headers: Range
    /// Access-Control-Allow-Headers: User-Agent
    /// Access-Control-Allow-Headers: X-Requested-With
    /// Access-Control-Allow-Methods: GET
    /// Access-Control-Allow-Origin: *
    /// Access-Control-Expose-Headers: Content-Length
    /// Access-Control-Expose-Headers: Content-Range
    /// Access-Control-Expose-Headers: X-Chunked-Output
    /// Access-Control-Expose-Headers: X-Ipfs-Path
    /// Access-Control-Expose-Headers: X-Ipfs-Roots
    /// Access-Control-Expose-Headers: X-Stream-Output
    /// Content-Type: text/html; charset=utf-8
    /// Location: /ipfs/QmVturFGV3z4WsP7cRV8Ci4avCdGWYXk2qBKvtAwFUp5Az/
    /// X-Ipfs-Path: /ipfs/QmVturFGV3z4WsP7cRV8Ci4avCdGWYXk2qBKvtAwFUp5Az
    /// X-Ipfs-Roots: QmVturFGV3z4WsP7cRV8Ci4avCdGWYXk2qBKvtAwFUp5Az
    /// Date: Thu, 08 Sep 2022 00:52:29 GMT
    /// ```
    ///
    /// In conclusion:
    ///
    /// - HTTP Status Code == 302 => directory
    /// - HTTP Status Code == 200 && ETag starts with `"DirIndex` => directory
    /// - HTTP Status Code == 200 && ETag not starts with `"DirIndex` => file
    pub async fn ipfs_stat(&self, ctx: &OperationContext, path: &str) -> Result<Metadata> {
        // Stat root always returns a DIR.
        if path == "/" {
            return Ok(MetadataBuilder::dir().build());
        }

        let resp = self.ipfs_head(ctx, path).await?;
        let status = resp.status();

        match status {
            StatusCode::OK => {
                let mut m = MetadataBuilder::unknown();

                let content_length = parse_content_length(resp.headers())?;

                if let Some(v) = parse_content_type(resp.headers())? {
                    m.content_type(v);
                }

                if let Some(v) = parse_etag(resp.headers())? {
                    m.etag(v);

                    if v.starts_with("\"DirIndex") {
                        m.set_dir();
                    } else {
                        m.set_file(content_length.ok_or_else(|| {
                            Error::new(
                                ErrorKind::Unexpected,
                                "IPFS response does not contain file content length",
                            )
                        })?);
                    }
                } else {
                    // Some service will stream the output of DirIndex.
                    // If we don't have an etag, it's highly to be a dir.
                    m.set_dir();
                }

                if let Some(v) = parse_content_disposition(resp.headers())? {
                    m.content_disposition(v);
                }

                Ok(m.build())
            }
            StatusCode::FOUND | StatusCode::MOVED_PERMANENTLY => Ok(MetadataBuilder::dir().build()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Resolve")),
                resp,
            )),
        }
    }
}

/// Context needed to classify an error from this service.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ErrorContext {
    service_operation: ServiceOperation,
}

impl ErrorContext {
    pub(crate) const fn new(service_operation: ServiceOperation) -> Self {
        Self { service_operation }
    }
}

/// Parse an error response using its service request context.
pub(crate) fn parse_error(ctx: ErrorContext, resp: Response<Buffer>) -> Error {
    let (parts, body) = resp.into_parts();
    let bs = body.to_bytes();

    let (kind, retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT
        // IPFS Gateway will return `408 REQUEST_TIMEOUT` while `ipfs resolve -r` failed.
        | StatusCode::REQUEST_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let message = String::from_utf8_lossy(&bs);

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}
