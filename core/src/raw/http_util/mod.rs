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

//! http_util contains the util types and functions that used across OpenDAL.
//!
//! # NOTE
//!
//! This mod is not a part of OpenDAL's public API. We expose them out to make
//! it easier to develop services and layers outside opendal.

mod client;
pub use client::HttpClient;
pub use client::HttpFetch;

/// temporary client used by several features
#[allow(unused_imports)]
pub(crate) use client::GLOBAL_REQWEST_CLIENT;

mod body;
pub use body::HttpBody;

mod header;
pub use header::build_header_value;
pub use header::format_authorization_by_basic;
pub use header::format_authorization_by_bearer;
pub use header::format_content_md5;
pub use header::parse_content_disposition;
pub use header::parse_content_encoding;
pub use header::parse_content_length;
pub use header::parse_content_md5;
pub use header::parse_content_range;
pub use header::parse_content_type;
pub use header::parse_etag;
pub use header::parse_header_to_str;
pub use header::parse_into_metadata;
pub use header::parse_last_modified;
pub use header::parse_location;
pub use header::parse_multipart_boundary;
pub use header::parse_prefixed_headers;

mod uri;
pub use uri::percent_decode_path;
pub use uri::percent_encode_path;
pub use uri::query_pairs;

mod error;
pub use error::new_request_build_error;
pub use error::new_request_credential_error;
pub use error::new_request_sign_error;
pub use error::with_error_response_context;

mod bytes_range;
pub use bytes_range::BytesRange;

mod bytes_content_range;
pub use bytes_content_range::BytesContentRange;

mod multipart;
pub use multipart::FormDataPart;
pub use multipart::MixedPart;
pub use multipart::Multipart;
pub use multipart::Part;
