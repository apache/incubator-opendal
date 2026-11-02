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

use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use percent_encoding::percent_decode_str;

/// An HTTP URI or relative location with a precomputed diagnostic representation.
///
/// Construction preserves the original spelling and masks user information and
/// recognized credential query values in a separate representation. It does not
/// validate, resolve, or normalize the URI. Ordinary query values remain intact.
/// Clones share both representations; formatting does not repeat redaction.
///
/// Recognized query names are `Signature`, `Policy`, `token`, `access_token`,
/// `X-Amz-Signature`, and `X-Amz-Security-Token`, matched case-insensitively after
/// decoding the name. Arbitrary application parameters are not classified.
#[derive(Clone)]
pub struct HttpUri {
    original: Arc<str>,
    redacted: Arc<str>,
}

impl HttpUri {
    /// Retain the original URI and compute its diagnostic representation once.
    pub fn new(uri: impl Into<Arc<str>>) -> Self {
        let original = uri.into();
        let redacted = redact_uri(&original).into();
        Self { original, redacted }
    }

    /// Return the original URI, including any credentials it contains.
    pub fn original_uri(&self) -> &str {
        &self.original
    }

    /// Return the URI with recognized credentials masked for diagnostics.
    pub fn redacted_uri(&self) -> &str {
        &self.redacted
    }

    /// Get or create the diagnostic value for a response's `Location` header.
    ///
    /// Store it in response extensions so service errors and shared response
    /// diagnostics reuse the same value. If a wrapper changes the header,
    /// replace the cached value. Return `None` for missing or non-text headers.
    /// The original header is not changed.
    pub fn from_response_location(parts: &mut http::response::Parts) -> Option<&Self> {
        let location = parts.headers.get(http::header::LOCATION)?.to_str().ok()?;
        if parts
            .extensions
            .get::<Self>()
            .is_none_or(|uri| uri.original_uri() != location)
        {
            let uri = Self::new(location);
            parts.extensions.insert(uri);
        }
        parts.extensions.get::<Self>()
    }
}

impl Debug for HttpUri {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("HttpUri")
            .field(&self.redacted_uri())
            .finish()
    }
}

/// A reusable HTTP redirect handled by the transport.
///
/// A transport that supports this extension may return it with a response after
/// following redirects. Services can copy it into later request extensions to
/// reuse that destination while keeping the original request URI for diagnostics.
/// The transport must send to the destination and apply its redirect credential
/// policy, including credentials supplied by client defaults.
///
/// The destination retains its original and redacted representations in
/// [`HttpUri`]. Only the transport should use the original as a request URI.
/// Services may inspect it to determine whether reuse is valid, but must use
/// the redacted representation for diagnostics. This extension alone does not
/// establish that the original path still selects the same object. Transports
/// that do not support reusable redirects must not return it.
#[derive(Clone)]
pub struct HttpRedirect(HttpUri);

impl HttpRedirect {
    /// Record a destination that this transport can reuse.
    pub fn new(uri: http::Uri) -> Self {
        Self(HttpUri::new(uri.to_string()))
    }

    /// Return the destination's original and diagnostic representations.
    pub fn uri(&self) -> &HttpUri {
        &self.0
    }
}

impl Debug for HttpRedirect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("HttpRedirect")
            .field(&self.0.redacted_uri())
            .finish()
    }
}

fn redact_uri(value: &str) -> String {
    let mut value = value.to_string();
    let authority_start = value
        .split_once("://")
        .filter(|(scheme, _)| {
            scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
        })
        .map(|(scheme, _)| scheme.len() + 3)
        .or_else(|| value.starts_with("//").then_some(2));
    if let Some(start) = authority_start {
        let end = value[start..]
            .find(['/', '?', '#'])
            .map_or(value.len(), |offset| start + offset);
        if let Some(offset) = value[start..end].rfind('@') {
            value.replace_range(start..start + offset, "[REDACTED]");
        }
    }

    let end = value.find('#').unwrap_or(value.len());
    if let Some(start) = value[..end].find('?') {
        let query = value[start + 1..end]
            .split('&')
            .map(|pair| {
                let Some((key, _)) = pair.split_once('=') else {
                    return pair.to_string();
                };
                let name = percent_decode_str(key).decode_utf8_lossy();
                match name.to_ascii_lowercase().as_str() {
                    "signature"
                    | "policy"
                    | "token"
                    | "access_token"
                    | "x-amz-signature"
                    | "x-amz-security-token" => {
                        format!("{key}=[REDACTED]")
                    }
                    _ => pair.to_string(),
                }
            })
            .collect::<Vec<_>>()
            .join("&");
        value.replace_range(start + 1..end, &query);
    }
    value
}
