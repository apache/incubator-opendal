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
use opendal_core::raw::*;
use opendal_core::*;

/// Reader returned by this backend.
pub struct GithubReader {
    backend: GithubBackend,
    ctx: OperationContext,
    path: String,
}

impl GithubReader {
    pub(super) fn new(
        backend: GithubBackend,
        ctx: OperationContext,
        path: &str,
        _: OpRead,
    ) -> Self {
        Self {
            backend,
            ctx,
            path: path.to_string(),
        }
    }
}

impl oio::StreamRead for GithubReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();
        let resp = backend.core.get(&self.ctx, path, range).await?;

        let status = resp.status();

        let (rp, stream) = match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                let (part, mut body) = resp.into_parts();
                let meta = parse_into_metadata(path, &part.headers)?;

                // GitHub can ignore the Range header on authenticated requests
                // and return the full content with 200, so slice the body
                // client-side when the server did not honor the range.
                if status == StatusCode::PARTIAL_CONTENT || range.is_full() {
                    (
                        RpRead::new(meta),
                        Box::new(body) as Box<dyn oio::ReadStreamDyn>,
                    )
                } else {
                    let bs = body.to_buffer().await?;
                    let total_size = bs.len() as u64;
                    let sliced = bs.slice(range.to_content_range(bs.len())?);
                    let mut metadata = meta.into_builder();
                    metadata.set_file(total_size);
                    (
                        RpRead::new(metadata.build()),
                        Box::new(sliced) as Box<dyn oio::ReadStreamDyn>,
                    )
                }
            }
            _ => {
                let (part, mut body) = resp.into_parts();
                let buf = body.to_buffer().await?;
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("GetRawContent")),
                    Response::from_parts(part, buf),
                ));
            }
        };

        Ok((rp, stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Request;
    use opendal_core::raw::oio::{Read, ReadStream};

    #[derive(Debug)]
    struct FullBodyTransport;

    impl HttpTransport for FullBodyTransport {
        async fn fetch(&self, _req: Request<Buffer>) -> Result<Response<HttpBody>> {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("content-length", "10")
                .header("content-type", "text/plain")
                .header("etag", "\"fixture\"")
                .body(HttpBody::new(
                    futures::stream::iter([Ok(Buffer::from("0123456789"))]),
                    Some(10),
                ))
                .expect("response must build"))
        }
    }

    #[tokio::test]
    async fn test_ignored_range_preserves_metadata() {
        let service = GithubBuilder::default()
            .owner("owner")
            .repo("repo")
            .token("test-token")
            .build()
            .expect("github service must build");
        let ctx =
            OperationContext::new().with_http_transport(HttpTransporter::new(FullBodyTransport));
        let reader = service
            .read(&ctx, "file.txt", OpRead::default())
            .expect("reader must be created");

        for (range, expected) in [
            (BytesRange::new(2, Some(3)), b"234".as_slice()),
            (BytesRange::suffix(3), b"789".as_slice()),
        ] {
            let (reply, mut stream) = reader.open(range).await.expect("range must open");
            let metadata = reply.metadata().expect("read must return metadata");
            assert_eq!(metadata.content_length(), 10);
            assert_eq!(metadata.etag(), Some("\"fixture\""));
            assert_eq!(metadata.content_type(), Some("text/plain"));
            assert_eq!(
                stream.read().await.expect("range must read").to_vec(),
                expected
            );
            assert!(stream.read().await.expect("EOF must read").is_empty());
        }
    }
}
