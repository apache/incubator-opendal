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

use std::mem;
use std::str::FromStr;

use bytes::Bytes;
use bytes::BytesMut;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use http::Request;
use http::Response;
use http::StatusCode;
use http::Uri;
use http::Version;
use http::header::CONTENT_DISPOSITION;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::uri::PathAndQuery;

use super::new_request_build_error;
use crate::*;

/// Multipart is a builder for multipart/form-data.
#[derive(Debug)]
pub struct Multipart<T: Part> {
    boundary: String,
    parts: Vec<T>,
}

impl<T: Part> Default for Multipart<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Part> Multipart<T> {
    /// Create a new multipart with random boundary.
    pub fn new() -> Self {
        Multipart {
            boundary: format!("opendal-{}", uuid::Uuid::new_v4()),
            parts: Vec::default(),
        }
    }

    /// Set the boundary with given string.
    pub fn with_boundary(mut self, boundary: &str) -> Self {
        self.boundary = boundary.to_string();
        self
    }

    /// Insert a part into multipart.
    pub fn part(mut self, part: T) -> Self {
        self.parts.push(part);
        self
    }

    /// Into parts.
    pub fn into_parts(self) -> Vec<T> {
        self.parts
    }

    /// Parse a response with multipart body into Multipart.
    pub fn parse(mut self, bs: Bytes) -> Result<Self> {
        let s = String::from_utf8(bs.to_vec()).map_err(|err| {
            Error::new(
                ErrorKind::Unexpected,
                "multipart response contains invalid utf-8 chars",
            )
            .set_source(err)
        })?;

        let parts = s
            .split(format!("--{}", self.boundary).as_str())
            .collect::<Vec<&str>>();

        for part in parts {
            if part.is_empty() || part.starts_with("--") {
                continue;
            }

            self.parts.push(T::parse(part)?);
        }

        Ok(self)
    }

    pub(crate) fn build(self) -> Buffer {
        let mut bufs = Vec::with_capacity(self.parts.len() + 2);

        // Build pre part.
        let mut bs = BytesMut::new();
        bs.extend_from_slice(b"--");
        bs.extend_from_slice(self.boundary.as_bytes());
        bs.extend_from_slice(b"\r\n");
        let pre_part = Buffer::from(bs.freeze());

        // Write all parts.
        for part in self.parts {
            bufs.push(pre_part.clone());
            bufs.push(part.format());
        }

        // Write the last boundary
        let mut bs = BytesMut::new();
        bs.extend_from_slice(b"--");
        bs.extend_from_slice(self.boundary.as_bytes());
        bs.extend_from_slice(b"--");
        bs.extend_from_slice(b"\r\n");

        // Build final part.
        bufs.push(Buffer::from(bs.freeze()));

        bufs.into_iter().flatten().collect()
    }

    /// Consume the input and generate a request with multipart body.
    ///
    /// This function will make sure content_type and content_length set correctly.
    pub fn apply(self, mut builder: http::request::Builder) -> Result<Request<Buffer>> {
        let boundary = self.boundary.clone();
        let buf = self.build();
        let content_length = buf.len();

        // Insert content type with correct boundary.
        builder = builder.header(
            CONTENT_TYPE,
            format!("multipart/{}; boundary={}", T::TYPE, boundary).as_str(),
        );
        // Insert content length with calculated size.
        builder = builder.header(CONTENT_LENGTH, content_length);

        builder.body(buf).map_err(new_request_build_error)
    }
}

/// Part is a trait for multipart part.
pub trait Part: Sized + 'static {
    /// TYPE is the type of multipart.
    ///
    /// Current available types are: `form-data` and `mixed`
    const TYPE: &'static str;

    /// format will generates the bytes.
    fn format(self) -> Buffer;

    /// parse will parse the bytes into a part.
    fn parse(s: &str) -> Result<Self>;
}

/// FormDataPart is a builder for multipart/form-data part.
pub struct FormDataPart {
    headers: HeaderMap,

    content: Buffer,
}

impl FormDataPart {
    /// Create a new part builder
    ///
    /// # Panics
    ///
    /// Input name must be percent encoded.
    pub fn new(name: &str) -> Self {
        // Insert content disposition header for part.
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_DISPOSITION,
            format!("form-data; name=\"{name}\"").parse().unwrap(),
        );

        Self {
            headers,
            content: Buffer::new(),
        }
    }

    /// Insert a header into part.
    pub fn header(mut self, key: HeaderName, value: HeaderValue) -> Self {
        self.headers.insert(key, value);
        self
    }

    /// Set the content for this part.
    pub fn content(mut self, content: impl Into<Buffer>) -> Self {
        self.content = content.into();
        self
    }
}

impl Part for FormDataPart {
    const TYPE: &'static str = "form-data";

    fn format(self) -> Buffer {
        let mut bufs = Vec::with_capacity(3);

        // Building pre-content.
        let mut bs = BytesMut::new();
        for (k, v) in self.headers.iter() {
            // Trick!
            //
            // Seafile could not recognize header names like `content-disposition`
            // and requires to use `Content-Disposition`. So we hardcode the part
            // headers name here.
            match k.as_str() {
                "content-disposition" => {
                    bs.extend_from_slice("Content-Disposition".as_bytes());
                }
                _ => {
                    bs.extend_from_slice(k.as_str().as_bytes());
                }
            }
            bs.extend_from_slice(b": ");
            bs.extend_from_slice(v.as_bytes());
            bs.extend_from_slice(b"\r\n");
        }
        bs.extend_from_slice(b"\r\n");
        bufs.push(Buffer::from(bs.freeze()));

        // Building content.
        bufs.push(self.content);

        // Building post-content.
        bufs.push(Buffer::from("\r\n"));

        bufs.into_iter().flatten().collect()
    }

    fn parse(_: &str) -> Result<Self> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "parse of form-data is not supported",
        ))
    }
}

/// MixedPart is a builder for multipart/mixed part.
pub struct MixedPart {
    part_headers: HeaderMap,

    /// Common
    version: Version,
    headers: HeaderMap,
    content: Buffer,

    /// Request only
    method: Option<Method>,
    uri: Option<Uri>,

    /// Response only
    status_code: Option<StatusCode>,
}

impl MixedPart {
    /// Create a new mixed part with given uri.
    pub fn new(uri: &str) -> Self {
        let mut part_headers = HeaderMap::new();
        part_headers.insert(CONTENT_TYPE, "application/http".parse().unwrap());
        part_headers.insert("content-transfer-encoding", "binary".parse().unwrap());

        let uri = Uri::from_str(uri).expect("the uri used to build a mixed part must be valid");

        Self {
            part_headers,

            version: Version::HTTP_11,
            headers: HeaderMap::new(),
            content: Buffer::new(),

            uri: Some(uri),
            method: None,

            status_code: None,
        }
    }

    /// Build a mixed part from a request.
    pub fn from_request(req: Request<Buffer>) -> Self {
        let mut part_headers = HeaderMap::new();
        part_headers.insert(CONTENT_TYPE, "application/http".parse().unwrap());
        part_headers.insert("content-transfer-encoding", "binary".parse().unwrap());

        let (parts, content) = req.into_parts();

        Self {
            part_headers,
            uri: Some(
                Uri::from_str(
                    parts
                        .uri
                        .path_and_query()
                        .unwrap_or(&PathAndQuery::from_static("/"))
                        .as_str(),
                )
                .expect("the uri used to build a mixed part must be valid"),
            ),
            version: parts.version,
            headers: parts.headers,
            content,

            method: Some(parts.method),
            status_code: None,
        }
    }

    /// Consume a mixed part to build a response.
    pub fn into_response(mut self) -> Response<Buffer> {
        let mut builder = Response::builder();

        builder = builder.status(self.status_code.unwrap_or(StatusCode::OK));
        builder = builder.version(self.version);
        // Swap headers directly instead of copy the entire map.
        mem::swap(builder.headers_mut().unwrap(), &mut self.headers);

        builder
            .body(self.content)
            .expect("mixed part must be valid response")
    }

    /// Insert a part header into part.
    pub fn part_header(mut self, key: HeaderName, value: HeaderValue) -> Self {
        self.part_headers.insert(key, value);
        self
    }

    /// Set the method for request in this part.
    pub fn method(mut self, method: Method) -> Self {
        self.method = Some(method);
        self
    }

    /// Set the version for request in this part.
    pub fn version(mut self, version: Version) -> Self {
        self.version = version;
        self
    }

    /// Insert a header into part.
    pub fn header(mut self, key: HeaderName, value: HeaderValue) -> Self {
        self.headers.insert(key, value);
        self
    }

    /// Set the content for this part.
    pub fn content(mut self, content: impl Into<Buffer>) -> Self {
        self.content = content.into();
        self
    }
}

impl Part for MixedPart {
    const TYPE: &'static str = "mixed";

    fn format(self) -> Buffer {
        let mut bufs = Vec::with_capacity(3);

        // Write parts headers.
        let mut bs = BytesMut::new();
        for (k, v) in self.part_headers.iter() {
            // Trick!
            //
            // Azblob could not recognize header names like `content-type`
            // and requires to use `Content-Type`. So we hardcode the part
            // headers name here.
            match k.as_str() {
                "content-type" => {
                    bs.extend_from_slice("Content-Type".as_bytes());
                }
                "content-id" => {
                    bs.extend_from_slice("Content-ID".as_bytes());
                }
                "content-transfer-encoding" => {
                    bs.extend_from_slice("Content-Transfer-Encoding".as_bytes());
                }
                _ => {
                    bs.extend_from_slice(k.as_str().as_bytes());
                }
            }
            bs.extend_from_slice(b": ");
            bs.extend_from_slice(v.as_bytes());
            bs.extend_from_slice(b"\r\n");
        }

        // Write request line: `DELETE /container0/blob0 HTTP/1.1`
        bs.extend_from_slice(b"\r\n");
        bs.extend_from_slice(
            self.method
                .as_ref()
                .expect("mixed part must be a valid request that contains method")
                .as_str()
                .as_bytes(),
        );
        bs.extend_from_slice(b" ");
        let uri = self
            .uri
            .as_ref()
            .expect("mixed part must be a valid request that contains uri");
        bs.extend_from_slice(
            uri.path_and_query()
                .map_or_else(|| uri.path(), PathAndQuery::as_str)
                .as_bytes(),
        );
        bs.extend_from_slice(b" ");
        bs.extend_from_slice(format!("{:?}", self.version).as_bytes());
        bs.extend_from_slice(b"\r\n");

        // Write request headers.
        for (k, v) in self.headers.iter() {
            bs.extend_from_slice(k.as_str().as_bytes());
            bs.extend_from_slice(b": ");
            bs.extend_from_slice(v.as_bytes());
            bs.extend_from_slice(b"\r\n");
        }
        bs.extend_from_slice(b"\r\n");
        bufs.push(Buffer::from(bs.freeze()));

        if !self.content.is_empty() {
            bufs.push(self.content);
            bufs.push(Buffer::from("\r\n"))
        }

        bufs.into_iter().flatten().collect()
    }

    /// TODO
    ///
    /// This is a simple implementation and have a lot of space to improve.
    fn parse(s: &str) -> Result<Self> {
        let parts = s.splitn(2, "\r\n\r\n").collect::<Vec<&str>>();
        let part_headers_content = parts[0];
        let http_response = parts.get(1).unwrap_or(&"");

        let mut part_headers = HeaderMap::new();
        for line in part_headers_content.lines() {
            let parts = line.splitn(2, ": ").collect::<Vec<&str>>();
            if parts.len() == 2 {
                let header_name = HeaderName::from_str(parts[0]).map_err(|err| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "multipart response contains invalid part header name",
                    )
                    .set_source(err)
                })?;
                let header_value = parts[1].parse().map_err(|err| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "multipart response contains invalid part header value",
                    )
                    .set_source(err)
                })?;

                part_headers.insert(header_name, header_value);
            }
        }

        let parts = http_response.split("\r\n\r\n").collect::<Vec<&str>>();
        let headers_content = parts[0];
        let body_content = parts.get(1).unwrap_or(&"");
        let body_bytes = Buffer::from(body_content.as_bytes().to_vec());

        let status_line = headers_content.lines().next().unwrap_or("");
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .parse::<u16>()
            .unwrap_or(200);

        let mut headers = HeaderMap::new();
        for line in headers_content.lines().skip(1) {
            let parts = line.splitn(2, ": ").collect::<Vec<&str>>();
            if parts.len() == 2 {
                let header_name = HeaderName::from_str(parts[0]).map_err(|err| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "multipart response contains invalid part header name",
                    )
                    .set_source(err)
                })?;
                let header_value = parts[1].parse().map_err(|err| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "multipart response contains invalid part header value",
                    )
                    .set_source(err)
                })?;

                headers.insert(header_name, header_value);
            }
        }

        Ok(Self {
            part_headers,
            version: Version::HTTP_11,
            headers,
            content: body_bytes,

            method: None,
            uri: None,

            status_code: Some(StatusCode::from_u16(status_code).map_err(|err| {
                Error::new(
                    ErrorKind::Unexpected,
                    "multipart response contains invalid status code",
                )
                .set_source(err)
            })?),
        })
    }
}

/// RelatedPart is a builder for multipart/related part.
pub struct RelatedPart {
    /// Common
    headers: HeaderMap,
    content: Buffer,
}

impl Default for RelatedPart {
    fn default() -> Self {
        Self::new()
    }
}

impl RelatedPart {
    /// Create a new related
    pub fn new() -> Self {
        Self {
            headers: HeaderMap::new(),
            content: Buffer::new(),
        }
    }

    /// Build a mixed part from a request.
    pub fn from_request(req: Request<Buffer>) -> Self {
        let (parts, content) = req.into_parts();

        Self {
            headers: parts.headers,
            content,
        }
    }

    /// Consume a mixed part to build a response.
    pub fn into_response(mut self) -> Response<Buffer> {
        let mut builder = Response::builder();

        // Swap headers directly instead of copy the entire map.
        mem::swap(builder.headers_mut().unwrap(), &mut self.headers);

        builder
            .body(self.content)
            .expect("a related part must be valid response")
    }

    /// Insert a header into part.
    pub fn header(mut self, key: HeaderName, value: HeaderValue) -> Self {
        self.headers.insert(key, value);
        self
    }

    /// Set the content for this part.
    pub fn content(mut self, content: impl Into<Buffer>) -> Self {
        self.content = content.into();
        self
    }
}

impl Part for RelatedPart {
    const TYPE: &'static str = "related";

    fn format(self) -> Buffer {
        // This is what multipart/related body might look for an insert in GCS
        // https://cloud.google.com/storage/docs/uploading-objects
        /*
        --separator_string
        Content-Type: application/json; charset=UTF-8

        {"name":"my-document.txt"}

        --separator_string
        Content-Type: text/plain

        This is a text file.
        --separator_string--
        */

        let mut bufs = Vec::with_capacity(3);
        let mut bs = BytesMut::new();

        // Write request headers.
        for (k, v) in self.headers.iter() {
            bs.extend_from_slice(k.as_str().as_bytes());
            bs.extend_from_slice(b": ");
            bs.extend_from_slice(v.as_bytes());
            bs.extend_from_slice(b"\r\n");
        }
        bs.extend_from_slice(b"\r\n");
        bufs.push(Buffer::from(bs.freeze()));

        if !self.content.is_empty() {
            bufs.push(self.content);
            bufs.push(Buffer::from("\r\n"))
        }

        bufs.into_iter().flatten().collect()
    }

    fn parse(_s: &str) -> Result<Self> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "parsing multipart/related is not supported",
        ))
    }
}

#[cfg(test)]
mod tests {
    use http::header::CONTENT_TYPE;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_multipart_formdata_basic() -> Result<()> {
        let multipart = Multipart::new()
            .with_boundary("lalala")
            .part(FormDataPart::new("foo").content(Bytes::from("bar")))
            .part(FormDataPart::new("hello").content(Bytes::from("world")));

        let bs = multipart.build();

        let expected = "--lalala\r\n\
             Content-Disposition: form-data; name=\"foo\"\r\n\
             \r\n\
             bar\r\n\
             --lalala\r\n\
             Content-Disposition: form-data; name=\"hello\"\r\n\
             \r\n\
             world\r\n\
             --lalala--\r\n";

        assert_eq!(Bytes::from(expected), bs.to_bytes());
        Ok(())
    }

    /// This test is inspired by <https://docs.aws.amazon.com/AmazonS3/latest/userguide/HTTPPOSTExamples.html>
    #[test]
    fn test_multipart_formdata_s3_form_upload() -> Result<()> {
        let multipart = Multipart::new()
            .with_boundary("9431149156168")
            .part(FormDataPart::new("key").content("user/eric/MyPicture.jpg"))
            .part(FormDataPart::new("acl").content("public-read"))
            .part(FormDataPart::new("success_action_redirect").content(
                "https://awsexamplebucket1.s3.us-west-1.amazonaws.com/successful_upload.html",
            ))
            .part(FormDataPart::new("content-type").content("image/jpeg"))
            .part(FormDataPart::new("x-amz-meta-uuid").content("14365123651274"))
            .part(FormDataPart::new("x-amz-meta-tag").content("Some,Tag,For,Picture"))
            .part(FormDataPart::new("AWSAccessKeyId").content("AKIAIOSFODNN7EXAMPLE"))
            .part(FormDataPart::new("Policy").content("eyAiZXhwaXJhdGlvbiI6ICIyMDA3LTEyLTAxVDEyOjAwOjAwLjAwMFoiLAogICJjb25kaXRpb25zIjogWwogICAgeyJidWNrZXQiOiAiam9obnNtaXRoIn0sCiAgICBbInN0YXJ0cy13aXRoIiwgIiRrZXkiLCAidXNlci9lcmljLyJdLAogICAgeyJhY2wiOiAicHVibGljLXJlYWQifSwKICAgIHsic3VjY2Vzc19hY3Rpb25fcmVkaXJlY3QiOiAiaHR0cDovL2pvaG5zbWl0aC5zMy5hbWF6b25hd3MuY29tL3N1Y2Nlc3NmdWxfdXBsb2FkLmh0bWwifSwKICAgIFsic3RhcnRzLXdpdGgiLCAiJENvbnRlbnQtVHlwZSIsICJpbWFnZS8iXSwKICAgIHsieC1hbXotbWV0YS11dWlkIjogIjE0MzY1MTIzNjUxMjc0In0sCiAgICBbInN0YXJ0cy13aXRoIiwgIiR4LWFtei1tZXRhLXRhZyIsICIiXQogIF0KfQo="))
            .part(FormDataPart::new("Signature").content("0RavWzkygo6QX9caELEqKi9kDbU="))
            .part(FormDataPart::new("file").header(CONTENT_TYPE, "image/jpeg".parse().unwrap()).content("...file content...")).part(FormDataPart::new("submit").content("Upload to Amazon S3"));

        let bs = multipart.build();

        let expected = r#"--9431149156168
Content-Disposition: form-data; name="key"

user/eric/MyPicture.jpg
--9431149156168
Content-Disposition: form-data; name="acl"

public-read
--9431149156168
Content-Disposition: form-data; name="success_action_redirect"

https://awsexamplebucket1.s3.us-west-1.amazonaws.com/successful_upload.html
--9431149156168
Content-Disposition: form-data; name="content-type"

image/jpeg
--9431149156168
Content-Disposition: form-data; name="x-amz-meta-uuid"

14365123651274
--9431149156168
Content-Disposition: form-data; name="x-amz-meta-tag"

Some,Tag,For,Picture
--9431149156168
Content-Disposition: form-data; name="AWSAccessKeyId"

AKIAIOSFODNN7EXAMPLE
--9431149156168
Content-Disposition: form-data; name="Policy"

eyAiZXhwaXJhdGlvbiI6ICIyMDA3LTEyLTAxVDEyOjAwOjAwLjAwMFoiLAogICJjb25kaXRpb25zIjogWwogICAgeyJidWNrZXQiOiAiam9obnNtaXRoIn0sCiAgICBbInN0YXJ0cy13aXRoIiwgIiRrZXkiLCAidXNlci9lcmljLyJdLAogICAgeyJhY2wiOiAicHVibGljLXJlYWQifSwKICAgIHsic3VjY2Vzc19hY3Rpb25fcmVkaXJlY3QiOiAiaHR0cDovL2pvaG5zbWl0aC5zMy5hbWF6b25hd3MuY29tL3N1Y2Nlc3NmdWxfdXBsb2FkLmh0bWwifSwKICAgIFsic3RhcnRzLXdpdGgiLCAiJENvbnRlbnQtVHlwZSIsICJpbWFnZS8iXSwKICAgIHsieC1hbXotbWV0YS11dWlkIjogIjE0MzY1MTIzNjUxMjc0In0sCiAgICBbInN0YXJ0cy13aXRoIiwgIiR4LWFtei1tZXRhLXRhZyIsICIiXQogIF0KfQo=
--9431149156168
Content-Disposition: form-data; name="Signature"

0RavWzkygo6QX9caELEqKi9kDbU=
--9431149156168
Content-Disposition: form-data; name="file"
content-type: image/jpeg

...file content...
--9431149156168
Content-Disposition: form-data; name="submit"

Upload to Amazon S3
--9431149156168--
"#;

        assert_eq!(
            expected,
            // Rust can't represent `\r` in a string literal, so we
            // replace `\r\n` with `\n` for comparison
            String::from_utf8(bs.to_bytes().to_vec())
                .unwrap()
                .replace("\r\n", "\n")
        );

        Ok(())
    }

    /// This test is inspired by <https://cloud.google.com/storage/docs/batch>
    #[test]
    fn test_multipart_mixed_gcs_batch_metadata() -> Result<()> {
        let multipart = Multipart::new()
            .with_boundary("===============7330845974216740156==")
            .part(
                MixedPart::new("/storage/v1/b/example-bucket/o/obj1")
                    .method(Method::PATCH)
                    .part_header(
                        "content-id".parse().unwrap(),
                        "<b29c5de2-0db4-490b-b421-6a51b598bd22+1>".parse().unwrap(),
                    )
                    .header(
                        "content-type".parse().unwrap(),
                        "application/json".parse().unwrap(),
                    )
                    .header(
                        "accept".parse().unwrap(),
                        "application/json".parse().unwrap(),
                    )
                    .header("content-length".parse().unwrap(), "31".parse().unwrap())
                    .content(r#"{"metadata": {"type": "tabby"}}"#),
            )
            .part(
                MixedPart::new("/storage/v1/b/example-bucket/o/obj2")
                    .method(Method::PATCH)
                    .part_header(
                        "content-id".parse().unwrap(),
                        "<b29c5de2-0db4-490b-b421-6a51b598bd22+2>".parse().unwrap(),
                    )
                    .header(
                        "content-type".parse().unwrap(),
                        "application/json".parse().unwrap(),
                    )
                    .header(
                        "accept".parse().unwrap(),
                        "application/json".parse().unwrap(),
                    )
                    .header("content-length".parse().unwrap(), "32".parse().unwrap())
                    .content(r#"{"metadata": {"type": "tuxedo"}}"#),
            )
            .part(
                MixedPart::new("/storage/v1/b/example-bucket/o/obj3")
                    .method(Method::PATCH)
                    .part_header(
                        "content-id".parse().unwrap(),
                        "<b29c5de2-0db4-490b-b421-6a51b598bd22+3>".parse().unwrap(),
                    )
                    .header(
                        "content-type".parse().unwrap(),
                        "application/json".parse().unwrap(),
                    )
                    .header(
                        "accept".parse().unwrap(),
                        "application/json".parse().unwrap(),
                    )
                    .header("content-length".parse().unwrap(), "32".parse().unwrap())
                    .content(r#"{"metadata": {"type": "calico"}}"#),
            );

        let bs = multipart.build();

        let expected = r#"--===============7330845974216740156==
Content-Type: application/http
Content-Transfer-Encoding: binary
Content-ID: <b29c5de2-0db4-490b-b421-6a51b598bd22+1>

PATCH /storage/v1/b/example-bucket/o/obj1 HTTP/1.1
content-type: application/json
accept: application/json
content-length: 31

{"metadata": {"type": "tabby"}}
--===============7330845974216740156==
Content-Type: application/http
Content-Transfer-Encoding: binary
Content-ID: <b29c5de2-0db4-490b-b421-6a51b598bd22+2>

PATCH /storage/v1/b/example-bucket/o/obj2 HTTP/1.1
content-type: application/json
accept: application/json
content-length: 32

{"metadata": {"type": "tuxedo"}}
--===============7330845974216740156==
Content-Type: application/http
Content-Transfer-Encoding: binary
Content-ID: <b29c5de2-0db4-490b-b421-6a51b598bd22+3>

PATCH /storage/v1/b/example-bucket/o/obj3 HTTP/1.1
content-type: application/json
accept: application/json
content-length: 32

{"metadata": {"type": "calico"}}
--===============7330845974216740156==--
"#;

        assert_eq!(
            expected,
            // Rust can't represent `\r` in a string literal, so we
            // replace `\r\n` with `\n` for comparison
            String::from_utf8(bs.to_bytes().to_vec())
                .unwrap()
                .replace("\r\n", "\n")
        );

        Ok(())
    }

    #[test]
    fn test_multipart_mixed_request_line_keeps_query() {
        let req = Request::delete(
            "https://storage.googleapis.com/storage/v1/b/example-bucket/o/obj1?generation=123",
        )
        .body(Buffer::new())
        .unwrap();
        let multipart = Multipart::new()
            .with_boundary("batch")
            .part(MixedPart::from_request(req));

        let bs = String::from_utf8(multipart.build().to_bytes().to_vec()).unwrap();

        assert!(bs.contains(
            "\r\nDELETE /storage/v1/b/example-bucket/o/obj1?generation=123 HTTP/1.1\r\n"
        ));
    }

    /// This test is inspired by <https://learn.microsoft.com/en-us/rest/api/storageservices/blob-batch?tabs=azure-ad>
    #[test]
    fn test_multipart_mixed_azblob_batch_delete() -> Result<()> {
        let multipart = Multipart::new()
            .with_boundary("batch_357de4f7-6d0b-4e02-8cd2-6361411a9525")
            .part(
                MixedPart::new("/container0/blob0")
                    .method(Method::DELETE)
                    .part_header("content-id".parse().unwrap(), "0".parse().unwrap())
                    .header(
                        "x-ms-date".parse().unwrap(),
                        "Thu, 14 Jun 2018 16:46:54 GMT".parse().unwrap(),
                    )
                    .header(
                        "authorization".parse().unwrap(),
                        "SharedKey account:G4jjBXA7LI/RnWKIOQ8i9xH4p76pAQ+4Fs4R1VxasaE="
                            .parse()
                            .unwrap(),
                    )
                    .header("content-length".parse().unwrap(), "0".parse().unwrap()),
            )
            .part(
                MixedPart::new("/container1/blob1")
                    .method(Method::DELETE)
                    .part_header("content-id".parse().unwrap(), "1".parse().unwrap())
                    .header(
                        "x-ms-date".parse().unwrap(),
                        "Thu, 14 Jun 2018 16:46:54 GMT".parse().unwrap(),
                    )
                    .header(
                        "authorization".parse().unwrap(),
                        "SharedKey account:IvCoYDQ+0VcaA/hKFjUmQmIxXv2RT3XwwTsOTHL39HI="
                            .parse()
                            .unwrap(),
                    )
                    .header("content-length".parse().unwrap(), "0".parse().unwrap()),
            )
            .part(
                MixedPart::new("/container2/blob2")
                    .method(Method::DELETE)
                    .part_header("content-id".parse().unwrap(), "2".parse().unwrap())
                    .header(
                        "x-ms-date".parse().unwrap(),
                        "Thu, 14 Jun 2018 16:46:54 GMT".parse().unwrap(),
                    )
                    .header(
                        "authorization".parse().unwrap(),
                        "SharedKey account:S37N2JTjcmOQVLHLbDmp2johz+KpTJvKhbVc4M7+UqI="
                            .parse()
                            .unwrap(),
                    )
                    .header("content-length".parse().unwrap(), "0".parse().unwrap()),
            );

        let bs = multipart.build();

        let expected = r#"--batch_357de4f7-6d0b-4e02-8cd2-6361411a9525
Content-Type: application/http
Content-Transfer-Encoding: binary
Content-ID: 0

DELETE /container0/blob0 HTTP/1.1
x-ms-date: Thu, 14 Jun 2018 16:46:54 GMT
authorization: SharedKey account:G4jjBXA7LI/RnWKIOQ8i9xH4p76pAQ+4Fs4R1VxasaE=
content-length: 0

--batch_357de4f7-6d0b-4e02-8cd2-6361411a9525
Content-Type: application/http
Content-Transfer-Encoding: binary
Content-ID: 1

DELETE /container1/blob1 HTTP/1.1
x-ms-date: Thu, 14 Jun 2018 16:46:54 GMT
authorization: SharedKey account:IvCoYDQ+0VcaA/hKFjUmQmIxXv2RT3XwwTsOTHL39HI=
content-length: 0

--batch_357de4f7-6d0b-4e02-8cd2-6361411a9525
Content-Type: application/http
Content-Transfer-Encoding: binary
Content-ID: 2

DELETE /container2/blob2 HTTP/1.1
x-ms-date: Thu, 14 Jun 2018 16:46:54 GMT
authorization: SharedKey account:S37N2JTjcmOQVLHLbDmp2johz+KpTJvKhbVc4M7+UqI=
content-length: 0

--batch_357de4f7-6d0b-4e02-8cd2-6361411a9525--
"#;

        assert_eq!(
            expected,
            // Rust can't represent `\r` in a string literal, so we
            // replace `\r\n` with `\n` for comparison
            String::from_utf8(bs.to_bytes().to_vec())
                .unwrap()
                .replace("\r\n", "\n")
        );

        Ok(())
    }

    /// This test is inspired by <https://cloud.google.com/storage/docs/batch>
    #[test]
    fn test_multipart_mixed_gcs_batch_metadata_response() {
        let response = r#"--batch_pK7JBAk73-E=_AA5eFwv4m2Q=
Content-Type: application/http
Content-ID: <response-b29c5de2-0db4-490b-b421-6a51b598bd22+1>

HTTP/1.1 200 OK
ETag: "lGaP-E0memYDumK16YuUDM_6Gf0/V43j6azD55CPRGb9b6uytDYl61Y"
Content-Type: application/json; charset=UTF-8
Date: Mon, 22 Jan 2018 18:56:00 GMT
Expires: Mon, 22 Jan 2018 18:56:00 GMT
Cache-Control: private, max-age=0
Content-Length: 846

{"kind": "storage#object","id": "example-bucket/obj1/1495822576643790","metadata": {"type": "tabby"}}

--batch_pK7JBAk73-E=_AA5eFwv4m2Q=
Content-Type: application/http
Content-ID: <response-b29c5de2-0db4-490b-b421-6a51b598bd22+2>

HTTP/1.1 200 OK
ETag: "lGaP-E0memYDumK16YuUDM_6Gf0/91POdd-sxSAkJnS8Dm7wMxBSDKk"
Content-Type: application/json; charset=UTF-8
Date: Mon, 22 Jan 2018 18:56:00 GMT
Expires: Mon, 22 Jan 2018 18:56:00 GMT
Cache-Control: private, max-age=0
Content-Length: 846

{"kind": "storage#object","id": "example-bucket/obj2/1495822576643790","metadata": {"type": "tuxedo"}}

--batch_pK7JBAk73-E=_AA5eFwv4m2Q=
Content-Type: application/http
Content-ID: <response-b29c5de2-0db4-490b-b421-6a51b598bd22+3>

HTTP/1.1 200 OK
ETag: "lGaP-E0memYDumK16YuUDM_6Gf0/d2Z1F1_ZVbB1dC0YKM9rX5VAgIQ"
Content-Type: application/json; charset=UTF-8
Date: Mon, 22 Jan 2018 18:56:00 GMT
Expires: Mon, 22 Jan 2018 18:56:00 GMT
Cache-Control: private, max-age=0
Content-Length: 846

{"kind": "storage#object","id": "example-bucket/obj3/1495822576643790","metadata": {"type": "calico"}}

--batch_pK7JBAk73-E=_AA5eFwv4m2Q=--"#.replace('\n', "\r\n");

        let multipart: Multipart<MixedPart> = Multipart::new()
            .with_boundary("batch_pK7JBAk73-E=_AA5eFwv4m2Q=")
            .parse(Bytes::from(response))
            .unwrap();

        let part0_bs = Bytes::from_static(
            r#"{"kind": "storage#object","id": "example-bucket/obj1/1495822576643790","metadata": {"type": "tabby"}}"#.as_bytes());
        let part1_bs = Bytes::from_static(
            r#"{"kind": "storage#object","id": "example-bucket/obj2/1495822576643790","metadata": {"type": "tuxedo"}}"#
                .as_bytes()
        );
        let part2_bs = Bytes::from_static(
            r#"{"kind": "storage#object","id": "example-bucket/obj3/1495822576643790","metadata": {"type": "calico"}}"#
                .as_bytes()
        );

        assert_eq!(multipart.parts.len(), 3);

        assert_eq!(multipart.parts[0].part_headers, {
            let mut h = HeaderMap::new();
            h.insert("Content-Type", "application/http".parse().unwrap());
            h.insert(
                "Content-ID",
                "<response-b29c5de2-0db4-490b-b421-6a51b598bd22+1>"
                    .parse()
                    .unwrap(),
            );

            h
        });
        assert_eq!(multipart.parts[0].version, Version::HTTP_11);
        assert_eq!(multipart.parts[0].headers, {
            let mut h = HeaderMap::new();
            h.insert(
                "ETag",
                "\"lGaP-E0memYDumK16YuUDM_6Gf0/V43j6azD55CPRGb9b6uytDYl61Y\""
                    .parse()
                    .unwrap(),
            );
            h.insert(
                "Content-Type",
                "application/json; charset=UTF-8".parse().unwrap(),
            );
            h.insert("Date", "Mon, 22 Jan 2018 18:56:00 GMT".parse().unwrap());
            h.insert("Expires", "Mon, 22 Jan 2018 18:56:00 GMT".parse().unwrap());
            h.insert("Cache-Control", "private, max-age=0".parse().unwrap());
            h.insert("Content-Length", "846".parse().unwrap());

            h
        });
        assert_eq!(multipart.parts[0].content.len(), part0_bs.len());
        assert_eq!(multipart.parts[0].uri, None);
        assert_eq!(multipart.parts[0].method, None);
        assert_eq!(
            multipart.parts[0].status_code,
            Some(StatusCode::from_u16(200).unwrap())
        );

        assert_eq!(multipart.parts[1].part_headers, {
            let mut h = HeaderMap::new();
            h.insert("Content-Type", "application/http".parse().unwrap());
            h.insert(
                "Content-ID",
                "<response-b29c5de2-0db4-490b-b421-6a51b598bd22+2>"
                    .parse()
                    .unwrap(),
            );

            h
        });
        assert_eq!(multipart.parts[1].version, Version::HTTP_11);
        assert_eq!(multipart.parts[1].headers, {
            let mut h = HeaderMap::new();
            h.insert(
                "ETag",
                "\"lGaP-E0memYDumK16YuUDM_6Gf0/91POdd-sxSAkJnS8Dm7wMxBSDKk\""
                    .parse()
                    .unwrap(),
            );
            h.insert(
                "Content-Type",
                "application/json; charset=UTF-8".parse().unwrap(),
            );
            h.insert("Date", "Mon, 22 Jan 2018 18:56:00 GMT".parse().unwrap());
            h.insert("Expires", "Mon, 22 Jan 2018 18:56:00 GMT".parse().unwrap());
            h.insert("Cache-Control", "private, max-age=0".parse().unwrap());
            h.insert("Content-Length", "846".parse().unwrap());

            h
        });
        assert_eq!(multipart.parts[1].content.len(), part1_bs.len());
        assert_eq!(multipart.parts[1].uri, None);
        assert_eq!(multipart.parts[1].method, None);
        assert_eq!(
            multipart.parts[1].status_code,
            Some(StatusCode::from_u16(200).unwrap())
        );

        assert_eq!(multipart.parts[2].part_headers, {
            let mut h = HeaderMap::new();
            h.insert("Content-Type", "application/http".parse().unwrap());
            h.insert(
                "Content-ID",
                "<response-b29c5de2-0db4-490b-b421-6a51b598bd22+3>"
                    .parse()
                    .unwrap(),
            );

            h
        });
        assert_eq!(multipart.parts[2].version, Version::HTTP_11);
        assert_eq!(multipart.parts[2].headers, {
            let mut h = HeaderMap::new();
            h.insert(
                "ETag",
                "\"lGaP-E0memYDumK16YuUDM_6Gf0/d2Z1F1_ZVbB1dC0YKM9rX5VAgIQ\""
                    .parse()
                    .unwrap(),
            );
            h.insert(
                "Content-Type",
                "application/json; charset=UTF-8".parse().unwrap(),
            );
            h.insert("Date", "Mon, 22 Jan 2018 18:56:00 GMT".parse().unwrap());
            h.insert("Expires", "Mon, 22 Jan 2018 18:56:00 GMT".parse().unwrap());
            h.insert("Cache-Control", "private, max-age=0".parse().unwrap());
            h.insert("Content-Length", "846".parse().unwrap());

            h
        });
        assert_eq!(multipart.parts[2].content.len(), part2_bs.len());
        assert_eq!(multipart.parts[2].uri, None);
        assert_eq!(multipart.parts[2].method, None);
        assert_eq!(
            multipart.parts[2].status_code,
            Some(StatusCode::from_u16(200).unwrap())
        );
    }

    #[test]
    fn test_multipart_related_gcs_simple() {
        // This is what multipart/related body might look for an insert in GCS
        // https://cloud.google.com/storage/docs/uploading-objects
        let expected = r#"--separator_string
content-type: application/json; charset=UTF-8

{"name":"my-document.txt"}
--separator_string
content-type: text/plain

This is a text file.
--separator_string--
"#;

        let multipart = Multipart::new()
            .with_boundary("separator_string")
            .part(
                RelatedPart::new()
                    .header(
                        "Content-Type".parse().unwrap(),
                        "application/json; charset=UTF-8".parse().unwrap(),
                    )
                    .content(r#"{"name":"my-document.txt"}"#),
            )
            .part(
                RelatedPart::new()
                    .header(
                        "Content-Type".parse().unwrap(),
                        "text/plain".parse().unwrap(),
                    )
                    .content("This is a text file."),
            );

        let bs = multipart.build();

        let output = String::from_utf8(bs.to_bytes().to_vec())
            .unwrap()
            .replace("\r\n", "\n");

        assert_eq!(output, expected);
    }
}
