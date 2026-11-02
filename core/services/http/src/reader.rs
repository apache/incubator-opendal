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
pub struct HttpReader {
    backend: HttpBackend,
    ctx: OperationContext,
    path: String,
    args: OpRead,
}

impl HttpReader {
    pub(super) fn new(
        backend: HttpBackend,
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

impl oio::StreamRead for HttpReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();
        let args = self.args.clone();
        let resp = backend.core.http_get(&self.ctx, path, range, &args).await?;

        let status = resp.status();

        let (rp, stream) = match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                (read_metadata(path, resp.headers())?, resp.into_body())
            }
            _ => {
                let (part, mut body) = resp.into_parts();
                let buf = body.to_buffer().await?;
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("Get")),
                    Response::from_parts(part, buf),
                ));
            }
        };

        Ok((rp, Box::new(stream) as Box<dyn oio::ReadStreamDyn>))
    }
}

/// Build the read reply from a successful response's headers.
///
/// `Content-Length` is optional in HTTP: an HTTP/1.1 response can be framed
/// with `Transfer-Encoding: chunked` (RFC 9112 section 6.1) and an HTTP/2
/// response is framed by the end of the stream (RFC 9113 section 8.1.1), so
/// neither carries one. RFC 9110 section 8.6 only says a server *should* send
/// it when the length is known in advance.
///
/// `parse_into_metadata` requires a length for a file, because `Metadata` for a
/// file always carries its complete content length. That requirement is correct
/// for `stat`, but a read does not need it: `RpRead` holds an
/// `Option<Metadata>` precisely so a service can decline to report metadata
/// that the read did not observe natively. So when the response states no
/// length, report no metadata and let the body stream.
///
/// A caller that explicitly asks for `Reader::metadata()` then gets
/// `ErrorKind::Unsupported`, which is the documented answer for a service that
/// does not return metadata on read. Before this, the read itself failed.
fn read_metadata(path: &str, headers: &http::HeaderMap) -> Result<RpRead> {
    // Mirrors how parse_into_metadata derives the length, so the two cannot
    // disagree about whether one is present. A malformed header still errors
    // here rather than being silently treated as absent.
    let content_length = parse_content_range(headers)?
        .and_then(|value| value.size())
        .or(parse_content_length(headers)?);

    match content_length {
        Some(_) => Ok(RpRead::new(parse_into_metadata(path, headers)?)),
        None => Ok(RpRead::default()),
    }
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;
    use http::HeaderValue;

    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(
                http::header::HeaderName::from_bytes(name.as_bytes()).expect("name must be valid"),
                HeaderValue::from_str(value).expect("value must be valid"),
            );
        }
        headers
    }

    #[test]
    fn test_read_metadata_with_content_length() {
        let rp = read_metadata("file.txt", &headers(&[("content-length", "12")]))
            .expect("read must be accepted");

        let meta = rp.metadata().expect("metadata must be reported");
        assert_eq!(meta.content_length(), 12);
    }

    #[test]
    fn test_read_metadata_from_content_range() {
        // A ranged read states the full size in Content-Range, not
        // Content-Length, and that is still a length.
        let rp = read_metadata(
            "file.txt",
            &headers(&[("content-length", "4"), ("content-range", "bytes 0-3/12")]),
        )
        .expect("read must be accepted");

        let meta = rp.metadata().expect("metadata must be reported");
        assert_eq!(meta.content_length(), 12);
    }

    #[test]
    fn test_read_metadata_without_content_length_reports_none() {
        // A chunked HTTP/1.1 response, and the shape an HTTP/2 response takes:
        // no length anywhere. The read must still be accepted.
        let rp = read_metadata("file.txt", &headers(&[("transfer-encoding", "chunked")]))
            .expect("a response with no length must still be read");

        assert!(rp.metadata().is_none());
    }

    #[test]
    fn test_read_metadata_without_any_headers_reports_none() {
        let rp = read_metadata("file.txt", &HeaderMap::new())
            .expect("a response with no headers must still be read");

        assert!(rp.metadata().is_none());
    }

    #[test]
    fn test_read_metadata_rejects_a_malformed_content_length() {
        // Absent is not the same as unparsable: a server that sends garbage
        // should still be reported rather than treated as length-free.
        let err = read_metadata("file.txt", &headers(&[("content-length", "not-a-number")]))
            .expect_err("a malformed length must be an error");

        assert_eq!(err.kind(), ErrorKind::Unexpected);
    }
}
