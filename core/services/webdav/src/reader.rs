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

use super::backend::*;
use super::core::{ErrorContext, parse_error};
use http::Response;
use http::StatusCode;
use opendal_core::raw::oio;
use opendal_core::raw::*;
use opendal_core::*;

/// Reader returned by this backend.
pub struct WebdavReader {
    backend: WebdavBackend,
    ctx: OperationContext,
    path: String,
    args: OpRead,
}

impl WebdavReader {
    pub(super) fn new(
        backend: WebdavBackend,
        ctx: OperationContext,
        path: &str,
        args: OpRead,
    ) -> Self {
        Self {
            backend,
            ctx,
            path: path.to_string(),
            args,
        }
    }
}

/// Build the read reply from response headers.
///
/// Servers may omit `Content-Length` (for example with chunked transfer
/// encoding). The metadata is optional in `RpRead`, so return an empty reply
/// instead of failing the read when the object size is unknown.
fn parse_read_reply(path: &str, headers: &http::HeaderMap) -> Result<RpRead> {
    let size = parse_content_range(headers)?
        .and_then(|value| value.size())
        .or(parse_content_length(headers)?);
    if size.is_none() && !path.ends_with('/') {
        return Ok(RpRead::default());
    }
    Ok(RpRead::new(parse_into_metadata(path, headers)?))
}

impl oio::StreamRead for WebdavReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();
        let args = self.args.clone();
        let resp = backend
            .core
            .webdav_get(&self.ctx, path, range, &args)
            .await?;

        let status = resp.status();

        let (rp, stream) = match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                (parse_read_reply(path, resp.headers())?, resp.into_body())
            }
            _ => {
                let (part, mut body) = resp.into_parts();
                let buf = body.to_buffer().await?;
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("Get"))
                        .with_if_match(args.if_match().is_some()),
                    Response::from_parts(part, buf),
                ));
            }
        };

        Ok((rp, Box::new(stream) as Box<dyn oio::ReadStreamDyn>))
    }
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;
    use http::header::{CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE};

    use super::*;

    #[test]
    fn test_parse_read_reply_without_content_length() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "text/plain".parse().unwrap());

        let rp = parse_read_reply("file", &headers).unwrap();
        assert!(rp.metadata().is_none());
    }

    #[test]
    fn test_parse_read_reply_with_content_length() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_LENGTH, "5".parse().unwrap());

        let rp = parse_read_reply("file", &headers).unwrap();
        assert_eq!(rp.metadata().unwrap().content_length(), 5);
    }

    #[test]
    fn test_parse_read_reply_with_content_range() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_RANGE, "bytes 0-1/10".parse().unwrap());

        let rp = parse_read_reply("file", &headers).unwrap();
        assert_eq!(rp.metadata().unwrap().content_length(), 10);
    }

    #[test]
    fn test_parse_read_reply_rejects_invalid_content_length() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_LENGTH, "abc".parse().unwrap());

        assert!(parse_read_reply("file", &headers).is_err());
    }
}
