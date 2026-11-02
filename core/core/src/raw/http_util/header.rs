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

use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::header::CACHE_CONTROL;
use http::header::CONTENT_DISPOSITION;
use http::header::CONTENT_ENCODING;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_RANGE;
use http::header::CONTENT_TYPE;
use http::header::ETAG;
use http::header::LAST_MODIFIED;
use http::header::LOCATION;
use md5::Digest;

use crate::EntryMode;
use crate::Error;
use crate::ErrorKind;
use crate::Metadata;
use crate::MetadataBuilder;
use crate::Result;
use crate::raw::*;

/// Parse redirect location from header map
///
/// # Note
/// The returned value maybe a relative path, like `/index.html`, `/robots.txt`, etc.
pub fn parse_location(headers: &HeaderMap) -> Result<Option<&str>> {
    parse_header_to_str(headers, LOCATION)
}

/// Parse cache control from header map.
///
/// # Note
///
/// The returned value is the raw string of `cache-control` header,
/// maybe `no-cache`, `max-age=3600`, etc.
pub fn parse_cache_control(headers: &HeaderMap) -> Result<Option<&str>> {
    parse_header_to_str(headers, CACHE_CONTROL)
}

/// Parse content length from header map.
pub fn parse_content_length(headers: &HeaderMap) -> Result<Option<u64>> {
    parse_header_to_str(headers, CONTENT_LENGTH)?
        .map(|v| {
            v.parse::<u64>().map_err(|e| {
                Error::new(ErrorKind::Unexpected, "header value is not valid integer").set_source(e)
            })
        })
        .transpose()
}

/// Parse content md5 from header map.
pub fn parse_content_md5(headers: &HeaderMap) -> Result<Option<&str>> {
    parse_header_to_str(headers, "content-md5")
}

/// Parse content type from header map.
pub fn parse_content_type(headers: &HeaderMap) -> Result<Option<&str>> {
    parse_header_to_str(headers, CONTENT_TYPE)
}

/// Parse content encoding from header map.
pub fn parse_content_encoding(headers: &HeaderMap) -> Result<Option<&str>> {
    parse_header_to_str(headers, CONTENT_ENCODING)
}

/// Parse content range from header map.
pub fn parse_content_range(headers: &HeaderMap) -> Result<Option<BytesContentRange>> {
    parse_header_to_str(headers, CONTENT_RANGE)?
        .map(|v| v.parse())
        .transpose()
}

/// Parse last modified from header map.
pub fn parse_last_modified(headers: &HeaderMap) -> Result<Option<Timestamp>> {
    parse_header_to_str(headers, LAST_MODIFIED)?
        .map(Timestamp::parse_rfc2822)
        .transpose()
}

/// Parse etag from header map.
pub fn parse_etag(headers: &HeaderMap) -> Result<Option<&str>> {
    parse_header_to_str(headers, ETAG)
}

/// Parse Content-Disposition for header map
pub fn parse_content_disposition(headers: &HeaderMap) -> Result<Option<&str>> {
    parse_header_to_str(headers, CONTENT_DISPOSITION)
}

/// Parse multipart boundary from header map.
pub fn parse_multipart_boundary(headers: &HeaderMap) -> Result<Option<&str>> {
    parse_header_to_str(headers, CONTENT_TYPE).map(|v| v.and_then(|v| v.split("boundary=").nth(1)))
}

/// Parse header value to string according to name.
#[inline]
pub fn parse_header_to_str<K>(headers: &HeaderMap, name: K) -> Result<Option<&str>>
where
    HeaderName: TryFrom<K>,
{
    let name = HeaderName::try_from(name).map_err(|_| {
        Error::new(
            ErrorKind::Unexpected,
            "header name must be valid http header name but not",
        )
        .with_operation("http_util::parse_header_to_str")
    })?;

    let value = if let Some(v) = headers.get(&name) {
        v
    } else {
        return Ok(None);
    };

    Ok(Some(value.to_str().map_err(|e| {
        Error::new(
            ErrorKind::Unexpected,
            "header value must be valid utf-8 string but not",
        )
        .with_operation("http_util::parse_header_to_str")
        .with_context("header_name", name.as_str())
        .set_source(e)
    })?))
}

/// parse_into_metadata will parse standards http headers into Metadata.
///
/// # Notes
///
/// parse_into_metadata only handles the standard behavior of http
/// headers. If services have their own logic, they should update the parsed
/// metadata on demand.
pub fn parse_into_metadata(path: &str, headers: &HeaderMap) -> Result<Metadata> {
    let mode = if path.ends_with('/') {
        EntryMode::DIR
    } else {
        EntryMode::FILE
    };
    let content_length = parse_content_range(headers)?
        .and_then(|value| value.size())
        .or(parse_content_length(headers)?);
    let mut m = if mode == EntryMode::FILE {
        MetadataBuilder::file(content_length.ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "HTTP response does not contain file content length",
            )
        })?)
    } else {
        MetadataBuilder::dir()
    };

    if let Some(v) = parse_cache_control(headers)? {
        m.cache_control(v);
    }

    if let Some(v) = parse_content_type(headers)? {
        m.content_type(v);
    }

    if let Some(v) = parse_content_encoding(headers)? {
        m.content_encoding(v);
    }

    if let Some(v) = parse_etag(headers)? {
        m.etag(v);
    }

    if let Some(v) = parse_content_md5(headers)? {
        m.content_md5(v);
    }

    if let Some(v) = parse_last_modified(headers)? {
        m.last_modified(v);
    }

    if let Some(v) = parse_content_disposition(headers)? {
        m.content_disposition(v);
    }

    Ok(m.build())
}

/// Parse prefixed headers and return a map with the prefix of each header removed.
pub fn parse_prefixed_headers(headers: &HeaderMap, prefix: &str) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            name.as_str().strip_prefix(prefix).and_then(|stripped_key| {
                value
                    .to_str()
                    .ok()
                    .map(|parsed_value| (stripped_key.to_string(), parsed_value.to_string()))
            })
        })
        .collect()
}

/// format content md5 header by given input.
pub fn format_content_md5(bs: &[u8]) -> String {
    let mut hasher = md5::Md5::new();
    hasher.update(bs);

    general_purpose::STANDARD.encode(hasher.finalize())
}

/// format content md5 header by given iter of bytes.
pub fn format_content_md5_iter<I>(bs: I) -> String
where
    I: IntoIterator,
    I::Item: AsRef<[u8]>,
{
    let mut hasher = md5::Md5::new();

    for b in bs {
        hasher.update(b.as_ref());
    }

    general_purpose::STANDARD.encode(hasher.finalize())
}

/// format authorization header by basic auth.
///
/// # Errors
///
/// If input username is empty, function will return an unexpected error.
pub fn format_authorization_by_basic(username: &str, password: &str) -> Result<String> {
    if username.is_empty() {
        return Err(Error::new(
            ErrorKind::Unexpected,
            "can't build authorization header with empty username",
        ));
    }

    let value = general_purpose::STANDARD.encode(format!("{username}:{password}"));

    Ok(format!("Basic {value}"))
}

/// format authorization header by bearer token.
///
/// # Errors
///
/// If input token is empty, function will return an unexpected error.
pub fn format_authorization_by_bearer(token: &str) -> Result<String> {
    if token.is_empty() {
        return Err(Error::new(
            ErrorKind::Unexpected,
            "can't build authorization header with empty token",
        ));
    }

    Ok(format!("Bearer {token}"))
}

/// Build header value from given string.
pub fn build_header_value(v: &str) -> Result<HeaderValue> {
    HeaderValue::from_str(v).map_err(|e| {
        Error::new(
            ErrorKind::ConfigInvalid,
            "header value contains invalid characters",
        )
        .with_operation("http_util::build_header_value")
        .set_source(e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test cases is from https://docs.aws.amazon.com/AmazonS3/latest/API/API_DeleteObjects.html
    #[test]
    fn test_format_content_md5() {
        let cases = vec![(
            r#"<Delete>
<Object>
 <Key>sample1.txt</Key>
 </Object>
 <Object>
   <Key>sample2.txt</Key>
 </Object>
 </Delete>"#,
            "WOctCY1SS662e7ziElh4cw==",
        )];

        for (input, expected) in cases {
            let actual = format_content_md5(input.as_bytes());

            assert_eq!(actual, expected)
        }
    }

    /// Test cases is borrowed from
    ///
    /// - RFC2617: https://datatracker.ietf.org/doc/html/rfc2617#section-2
    /// - MDN: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Authorization
    #[test]
    fn test_format_authorization_by_basic() {
        let cases = vec![
            ("aladdin", "opensesame", "Basic YWxhZGRpbjpvcGVuc2VzYW1l"),
            ("aladdin", "", "Basic YWxhZGRpbjo="),
            (
                "Aladdin",
                "open sesame",
                "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==",
            ),
            ("Aladdin", "", "Basic QWxhZGRpbjo="),
        ];

        for (username, password, expected) in cases {
            let actual =
                format_authorization_by_basic(username, password).expect("format must success");

            assert_eq!(actual, expected)
        }
    }

    /// Test cases is borrowed from
    ///
    /// - RFC6750: https://datatracker.ietf.org/doc/html/rfc6750
    #[test]
    fn test_format_authorization_by_bearer() {
        let cases = vec![("mF_9.B5f-4.1JqM", "Bearer mF_9.B5f-4.1JqM")];

        for (token, expected) in cases {
            let actual = format_authorization_by_bearer(token).expect("format must success");

            assert_eq!(actual, expected)
        }
    }

    #[test]
    fn test_parse_multipart_boundary() {
        let cases = vec![
            (
                "multipart/mixed; boundary=gc0p4Jq0M2Yt08jU534c0p",
                Some("gc0p4Jq0M2Yt08jU534c0p"),
            ),
            ("multipart/mixed", None),
        ];

        for (input, expected) in cases {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_str(input).unwrap());

            let actual = parse_multipart_boundary(&headers).expect("parse must success");

            assert_eq!(actual, expected)
        }
    }
}
