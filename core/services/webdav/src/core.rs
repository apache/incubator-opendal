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
use std::collections::VecDeque;
use std::fmt::Debug;
use std::io::Cursor;

use bytes::Bytes;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use quick_xml::Reader;
use quick_xml::Writer;
use quick_xml::escape::escape;
use quick_xml::events::BytesEnd;
use quick_xml::events::BytesStart;
use quick_xml::events::BytesText;
use quick_xml::events::Event;
use serde::Deserialize;

use opendal_core::raw::*;
use opendal_core::*;

/// Default namespace prefix for user-defined properties in WebDAV.
/// Users can override this via configuration.
pub const DEFAULT_USER_METADATA_PREFIX: &str = "opendal";
/// Default namespace URI for user-defined properties in WebDAV.
/// Users can override this via configuration.
pub const DEFAULT_USER_METADATA_URI: &str = "https://opendal.apache.org/ns";

/// The request to query all properties of a file or directory.
///
/// rfc4918 9.1: retrieve all properties define in specification
static PROPFIND_REQUEST: &str = r#"<?xml version="1.0" encoding="utf-8" ?><D:propfind xmlns:D="DAV:"><D:allprop/></D:propfind>"#;

/// The header to specify the depth of the query.
///
/// Valid values are `0`, `1`, `infinity`.
///
/// - `0`: only to the resource itself.
/// - `1`: to the resource and its internal members only.
/// - `infinity`: to the resource and all its members.
///
/// reference: [RFC4918: 10.2. Depth Header](https://datatracker.ietf.org/doc/html/rfc4918#section-10.2)
static HEADER_DEPTH: &str = "Depth";
/// The header to specify the destination of the query.
///
/// The Destination request header specifies the URI that identifies a
/// destination resource for methods such as COPY and MOVE, which take
/// two URIs as parameters.
///
/// reference: [RFC4918: 10.3.  Destination Header](https://datatracker.ietf.org/doc/html/rfc4918#section-10.3)
static HEADER_DESTINATION: &str = "Destination";
/// The header to specify the overwrite behavior of the query
///
/// The Overwrite request header specifies whether the server should
/// overwrite a resource mapped to the destination URL during a COPY or
/// MOVE.
///
/// Valid values are `T` and `F`.
///
/// A value of "F" states that the server must not perform the COPY or MOVE operation
/// if the destination URL does map to a resource.
///
/// reference: [RFC4918: 10.6.  Overwrite Header](https://datatracker.ietf.org/doc/html/rfc4918#section-10.6)
static HEADER_OVERWRITE: &str = "Overwrite";

pub struct WebdavCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub endpoint: String,
    pub server_path: String,
    pub root: String,
    pub authorization: Option<String>,
    /// XML namespace prefix for user metadata properties.
    pub user_metadata_prefix: String,
    /// XML namespace URI for user metadata properties.
    pub user_metadata_uri: String,
    /// Skip automatic parent directory creation before writes.
    pub disable_create_dir: bool,
}

impl Debug for WebdavCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebdavCore")
            .field("endpoint", &self.endpoint)
            .field("root", &self.root)
            .field("user_metadata_prefix", &self.user_metadata_prefix)
            .field("user_metadata_uri", &self.user_metadata_uri)
            .field("disable_create_dir", &self.disable_create_dir)
            .finish_non_exhaustive()
    }
}

impl WebdavCore {
    pub async fn webdav_stat(&self, ctx: &OperationContext, path: &str) -> Result<Metadata> {
        let path = build_rooted_abs_path(&self.root, path);
        self.webdav_stat_rooted_abs_path(ctx, &path).await
    }

    /// Input path must be `rooted_abs_path`.
    async fn webdav_stat_rooted_abs_path(
        &self,
        ctx: &OperationContext,
        rooted_abs_path: &str,
    ) -> Result<Metadata> {
        let url = format!("{}{}", self.endpoint, percent_encode_path(rooted_abs_path));
        let mut req = Request::builder().method("PROPFIND").uri(url);

        req = req.header(header::CONTENT_TYPE, "application/xml");
        req = req.header(header::CONTENT_LENGTH, PROPFIND_REQUEST.len());
        if let Some(auth) = &self.authorization {
            req = req.header(header::AUTHORIZATION, auth);
        }

        // Only stat the resource itself.
        req = req.header(HEADER_DEPTH, "0");

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("Propfind"))
            .body(Buffer::from(Bytes::from(PROPFIND_REQUEST)))
            .map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;
        if !resp.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("Propfind")),
                resp,
            ));
        }

        let bs = resp.into_body();
        let xml_bytes = bs.to_bytes();
        let xml_str = String::from_utf8_lossy(&xml_bytes);

        let result: Multistatus = deserialize_multistatus(&xml_bytes)?;
        let propfind_resp = result.response.first().ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                "propfind response is empty, the resource is not exist",
            )
        })?;

        let mut metadata = parse_propstats(&propfind_resp.propstat)?.into_builder();

        // Parse user metadata from the raw XML response using configured namespace
        let user_metadata = parse_user_metadata_from_xml(&xml_str, &self.user_metadata_uri);
        if !user_metadata.is_empty() {
            metadata.user_metadata(user_metadata);
        }

        Ok(metadata.build())
    }

    pub async fn webdav_get(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Response<HttpBody>> {
        let path = build_rooted_abs_path(&self.root, path);
        let url: String = format!("{}{}", self.endpoint, percent_encode_path(&path));

        let mut req = Request::get(&url);

        if let Some(if_match) = args.if_match() {
            req = req.header(header::IF_MATCH, if_match);
        }

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(header::IF_NONE_MATCH, if_none_match);
        }

        if let Some(if_modified_since) = args.if_modified_since() {
            req = req.header(
                header::IF_MODIFIED_SINCE,
                if_modified_since.format_http_date(),
            );
        }

        if let Some(if_unmodified_since) = args.if_unmodified_since() {
            req = req.header(
                header::IF_UNMODIFIED_SINCE,
                if_unmodified_since.format_http_date(),
            );
        }

        if let Some(auth) = &self.authorization {
            req = req.header(header::AUTHORIZATION, auth.clone())
        }

        if !range.is_full() {
            req = req.header(header::RANGE, range.to_header());
        }

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("Get"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub async fn webdav_put(
        &self,
        ctx: &OperationContext,
        path: &str,
        size: Option<u64>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let path = build_rooted_abs_path(&self.root, path);
        let url = format!("{}{}", self.endpoint, percent_encode_path(&path));

        let mut req = Request::put(&url);

        if let Some(v) = &self.authorization {
            req = req.header(header::AUTHORIZATION, v)
        }

        if let Some(v) = size {
            req = req.header(header::CONTENT_LENGTH, v)
        }

        if let Some(v) = args.content_type() {
            req = req.header(header::CONTENT_TYPE, v)
        }

        if let Some(v) = args.content_disposition() {
            req = req.header(header::CONTENT_DISPOSITION, v)
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("Put"))
            .body(body)
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    /// Set user-defined metadata using WebDAV PROPPATCH method.
    ///
    /// This method uses the OpenDAL custom namespace to store user metadata
    /// as DAV dead properties. Each key-value pair in the user_metadata map
    /// is stored as a property with the configured namespace prefix.
    ///
    /// # Reference
    /// - [RFC4918: 9.2 PROPPATCH Method](https://datatracker.ietf.org/doc/html/rfc4918#section-9.2)
    pub async fn webdav_proppatch(
        &self,
        ctx: &OperationContext,
        path: &str,
        user_metadata: &HashMap<String, String>,
    ) -> Result<Response<Buffer>> {
        let path = build_rooted_abs_path(&self.root, path);
        let url = format!("{}{}", self.endpoint, percent_encode_path(&path));

        let mut req = Request::builder().method("PROPPATCH").uri(&url);

        req = req.header(header::CONTENT_TYPE, "application/xml; charset=utf-8");
        if let Some(auth) = &self.authorization {
            req = req.header(header::AUTHORIZATION, auth);
        }

        // Build the PROPPATCH XML request body using configured namespace
        let proppatch_body = build_proppatch_request(
            user_metadata,
            &self.user_metadata_prefix,
            &self.user_metadata_uri,
        );

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("Proppatch"))
            .body(Buffer::from(Bytes::from(proppatch_body)))
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn webdav_delete(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let path = build_rooted_abs_path(&self.root, path);
        let url = format!("{}{}", self.endpoint, percent_encode_path(&path));

        let mut req = Request::delete(&url);

        if let Some(auth) = &self.authorization {
            req = req.header(header::AUTHORIZATION, auth.clone())
        }

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("Delete"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn webdav_copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        // Make sure target's dir is exist.
        self.webdav_mkcol(ctx, get_parent(to)).await?;

        let source = build_rooted_abs_path(&self.root, from);
        let source_uri = format!("{}{}", self.endpoint, percent_encode_path(&source));

        let target = build_rooted_abs_path(&self.root, to);
        let target_uri = format!("{}{}", self.endpoint, percent_encode_path(&target));

        let mut req = Request::builder().method("COPY").uri(&source_uri);

        if let Some(auth) = &self.authorization {
            req = req.header(header::AUTHORIZATION, auth);
        }

        req = req.header(HEADER_DESTINATION, target_uri);
        req = req.header(HEADER_OVERWRITE, "T");

        let req = req
            .extension(Operation::Copy)
            .extension(ServiceOperation("Copy"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn webdav_move(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        if_not_exists: bool,
    ) -> Result<Response<Buffer>> {
        // Check if source file exists.
        let _ = self.webdav_stat(ctx, from).await?;
        // Make sure target's dir is exist.
        self.webdav_mkcol(ctx, get_parent(to)).await?;

        let source = build_rooted_abs_path(&self.root, from);
        let source_uri = format!("{}{}", self.endpoint, percent_encode_path(&source));

        let target = build_rooted_abs_path(&self.root, to);
        let target_uri = format!("{}{}", self.endpoint, percent_encode_path(&target));

        let mut req = Request::builder().method("MOVE").uri(&source_uri);

        if let Some(auth) = &self.authorization {
            req = req.header(header::AUTHORIZATION, auth);
        }

        req = req.header(HEADER_DESTINATION, target_uri);
        req = req.header(HEADER_OVERWRITE, if if_not_exists { "F" } else { "T" });

        let req = req
            .extension(Operation::Rename)
            .extension(ServiceOperation("Move"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn webdav_list(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpList,
    ) -> Result<Response<Buffer>> {
        let path = build_rooted_abs_path(&self.root, path);
        let url = format!("{}{}", self.endpoint, percent_encode_path(&path));

        let mut req = Request::builder().method("PROPFIND").uri(&url);

        req = req.header(header::CONTENT_TYPE, "application/xml");
        req = req.header(header::CONTENT_LENGTH, PROPFIND_REQUEST.len());
        if let Some(auth) = &self.authorization {
            req = req.header(header::AUTHORIZATION, auth);
        }

        if args.recursive() {
            req = req.header(HEADER_DEPTH, "infinity");
        } else {
            req = req.header(HEADER_DEPTH, "1");
        }

        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("Propfind"))
            .body(Buffer::from(Bytes::from(PROPFIND_REQUEST)))
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    /// Create dir recursively for given path.
    ///
    /// # Notes
    ///
    /// We only expose this method to the backend since there are dependencies on input path.
    pub async fn webdav_mkcol(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let path = build_rooted_abs_path(&self.root, path);
        let mut path = path.as_str();

        let mut dirs = VecDeque::default();

        loop {
            match self.webdav_stat_rooted_abs_path(ctx, path).await {
                // Dir exists, break the loop.
                Ok(_) => {
                    break;
                }
                // Dir not found, keep going.
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    dirs.push_front(path);
                    path = get_parent(path);
                }
                // Unexpected error found, return it.
                Err(err) => return Err(err),
            }

            if path == "/" {
                break;
            }
        }

        for dir in dirs {
            self.webdav_mkcol_rooted_abs_path(ctx, dir).await?;
        }
        Ok(())
    }

    /// Create a dir
    ///
    /// Input path must be `rooted_abs_path`
    ///
    /// Reference: [RFC4918: 9.3.1.  MKCOL Status Codes](https://datatracker.ietf.org/doc/html/rfc4918#section-9.3.1)
    async fn webdav_mkcol_rooted_abs_path(
        &self,
        ctx: &OperationContext,
        rooted_abs_path: &str,
    ) -> Result<()> {
        let url = format!("{}{}", self.endpoint, percent_encode_path(rooted_abs_path));

        let mut req = Request::builder().method("MKCOL").uri(&url);

        if let Some(auth) = &self.authorization {
            req = req.header(header::AUTHORIZATION, auth.clone())
        }

        let req = req
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("Mkcol"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;
        let status = resp.status();

        match status {
            // 201 (Created) - The collection was created.
            StatusCode::CREATED
            // 405 (Method Not Allowed) - MKCOL can only be executed on an unmapped URL.
            //
            // The MKCOL method can only be performed on a deleted or non-existent resource.
            // This error means the directory already exists which is allowed by create_dir.
            | StatusCode::METHOD_NOT_ALLOWED => {

                Ok(())
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Mkcol")),
                resp,
            )),
        }
    }
}

/// Build a PROPPATCH request body to set user-defined metadata.
///
/// The request uses the specified namespace to store metadata as dead properties
/// on the WebDAV server.
///
/// # Arguments
/// * `user_metadata` - Key-value pairs to store as properties
/// * `namespace_prefix` - XML namespace prefix (e.g., "opendal")
/// * `namespace_uri` - XML namespace URI (e.g., "https://opendal.apache.org/ns")
///
/// # Example output
/// ```xml
/// <?xml version="1.0" encoding="utf-8"?>
/// <D:propertyupdate xmlns:D="DAV:" xmlns:opendal="https://opendal.apache.org/ns">
///   <D:set>
///     <D:prop>
///       <opendal:key1>value1</opendal:key1>
///       <opendal:key2>value2</opendal:key2>
///     </D:prop>
///   </D:set>
/// </D:propertyupdate>
/// ```
pub fn build_proppatch_request(
    user_metadata: &HashMap<String, String>,
    namespace_prefix: &str,
    namespace_uri: &str,
) -> String {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // Write XML declaration
    writer
        .write_event(Event::Decl(quick_xml::events::BytesDecl::new(
            "1.0",
            Some("utf-8"),
            None,
        )))
        .expect("write xml decl");

    // Write <D:propertyupdate> with namespace declarations
    let mut propertyupdate = BytesStart::new("D:propertyupdate");
    propertyupdate.push_attribute(("xmlns:D", "DAV:"));
    propertyupdate.push_attribute((
        format!("xmlns:{}", namespace_prefix).as_str(),
        namespace_uri,
    ));
    writer
        .write_event(Event::Start(propertyupdate))
        .expect("write propertyupdate");

    // Write <D:set>
    writer
        .write_event(Event::Start(BytesStart::new("D:set")))
        .expect("write set");

    // Write <D:prop>
    writer
        .write_event(Event::Start(BytesStart::new("D:prop")))
        .expect("write prop");

    // Write each user metadata property
    for (key, value) in user_metadata {
        // Note: key needs to be escaped for XML tag name, value is handled by BytesText
        let escaped_key = escape(key);
        let tag_name = format!("{}:{}", namespace_prefix, escaped_key);
        writer
            .write_event(Event::Start(BytesStart::new(&tag_name)))
            .expect("write prop start");
        // BytesText::new expects unescaped content and will escape it automatically
        writer
            .write_event(Event::Text(BytesText::new(value)))
            .expect("write prop value");
        writer
            .write_event(Event::End(BytesEnd::new(&tag_name)))
            .expect("write prop end");
    }

    // Close tags
    writer
        .write_event(Event::End(BytesEnd::new("D:prop")))
        .expect("write prop end");
    writer
        .write_event(Event::End(BytesEnd::new("D:set")))
        .expect("write set end");
    writer
        .write_event(Event::End(BytesEnd::new("D:propertyupdate")))
        .expect("write propertyupdate end");

    String::from_utf8(writer.into_inner().into_inner()).expect("valid utf8")
}

/// Parse user metadata from the raw XML response using quick-xml Reader.
///
/// This function extracts properties in the specified namespace
/// from the PROPFIND response and returns them as a HashMap.
///
/// Note: WebDAV servers like Nextcloud/ownCloud may use dynamic namespace prefixes
/// (e.g., x1:, x2:) instead of the prefix we send. The namespace URI is the
/// reliable identifier, not the prefix.
///
/// # Arguments
/// * `xml` - The raw XML response string
/// * `namespace_uri` - The namespace URI to look for
pub fn parse_user_metadata_from_xml(xml: &str, namespace_uri: &str) -> HashMap<String, String> {
    let mut user_metadata = HashMap::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // Track namespace prefix -> URI mappings
    let mut ns_prefixes: HashMap<String, String> = HashMap::new();
    // Track which prefixes map to our target namespace URI
    let mut target_prefixes: Vec<String> = Vec::new();

    // Current element state for tracking property values
    let mut current_prop_key: Option<String> = None;
    let mut current_prop_value = String::new();

    let mut buf = Vec::new();
    loop {
        let event = reader.read_event_into(&mut buf);
        match event {
            Ok(Event::Start(ref e)) => {
                // Extract namespace declarations from attributes
                for attr in e.attributes().flatten() {
                    let key = attr.key.as_ref().to_string();
                    if key.starts_with("xmlns:") {
                        let prefix = key.strip_prefix("xmlns:").unwrap_or("").to_string();
                        let uri = attr.value.to_string();
                        if uri == namespace_uri && !target_prefixes.contains(&prefix) {
                            target_prefixes.push(prefix.clone());
                        }
                        ns_prefixes.insert(prefix, uri);
                    }
                }

                // Check if this element is in our target namespace
                let name = e.name().as_ref().to_string();
                if let Some(colon_pos) = name.find(':') {
                    let prefix = &name[..colon_pos];
                    let local_name = &name[colon_pos + 1..];
                    if target_prefixes.contains(&prefix.to_string()) {
                        current_prop_key = Some(local_name.to_string());
                        current_prop_value.clear();
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                // Extract namespace declarations from attributes
                for attr in e.attributes().flatten() {
                    let key = attr.key.as_ref().to_string();
                    if key.starts_with("xmlns:") {
                        let prefix = key.strip_prefix("xmlns:").unwrap_or("").to_string();
                        let uri = attr.value.to_string();
                        if uri == namespace_uri && !target_prefixes.contains(&prefix) {
                            target_prefixes.push(prefix.clone());
                        }
                        ns_prefixes.insert(prefix, uri);
                    }
                }

                // For Empty events (self-closing tags), immediately insert with empty value
                let name = e.name().as_ref().to_string();
                if let Some(colon_pos) = name.find(':') {
                    let prefix = &name[..colon_pos];
                    let local_name = &name[colon_pos + 1..];
                    if target_prefixes.contains(&prefix.to_string()) {
                        user_metadata.insert(local_name.to_string(), String::new());
                    }
                }
            }
            Ok(Event::Text(ref e)) if current_prop_key.is_some() => {
                // Text content - add directly (no escaping needed)
                current_prop_value.push_str(e.as_ref());
            }
            Ok(Event::GeneralRef(ref e)) if current_prop_key.is_some() => {
                // Handle XML entity references (e.g., &lt; &gt; &amp; &quot; &apos;)
                let decoded = match e.as_ref() {
                    "lt" => "<",
                    "gt" => ">",
                    "amp" => "&",
                    "quot" => "\"",
                    "apos" => "'",
                    _ => "", // Unknown entity, skip
                };
                current_prop_value.push_str(decoded);
            }
            Ok(Event::End(ref e)) => {
                let name = e.name().as_ref().to_string();
                if let Some(colon_pos) = name.find(':') {
                    let prefix = &name[..colon_pos];
                    let local_name = &name[colon_pos + 1..];
                    if target_prefixes.contains(&prefix.to_string())
                        && let Some(key) = current_prop_key.take()
                        && key == local_name
                    {
                        user_metadata.insert(key, current_prop_value.clone());
                        current_prop_value.clear();
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    user_metadata
}

/// Check if a PROPPATCH 207 Multi-Status response indicates success using quick-xml Reader.
///
/// A 207 status code only indicates that there is detailed information available,
/// not success or failure. We need to parse the response body to check if all
/// property updates were successful (status 200).
///
/// Returns Ok(()) if all properties were successfully updated, or an error
/// with details about the failure.
pub fn check_proppatch_response(xml: &str) -> Result<()> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut in_status = false;
    let mut status_text = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name().as_ref().to_lowercase();
                // Match status element regardless of namespace prefix
                if name.ends_with(":status") || name == "status" {
                    in_status = true;
                    status_text.clear();
                }
            }
            Ok(Event::Text(ref e)) if in_status => {
                status_text.push_str(e.as_ref());
            }
            Ok(Event::End(ref e)) => {
                let name = e.name().as_ref().to_lowercase();
                if name.ends_with(":status") || name == "status" {
                    // Parse status code from "HTTP/1.1 XXX Description"
                    if let Some(code_str) = status_text.split_whitespace().nth(1)
                        && let Ok(code) = code_str.parse::<u16>()
                        && !(200..300).contains(&code)
                    {
                        return Err(Error::new(
                            ErrorKind::Unexpected,
                            format!("PROPPATCH failed with status: {status_text}"),
                        ));
                    }
                    in_status = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(())
}

pub fn deserialize_multistatus(bs: &[u8]) -> Result<Multistatus> {
    let s = String::from_utf8_lossy(bs);
    // HACKS! HACKS! HACKS!
    //
    // Make sure the string is escaped.
    // Related to <https://github.com/tafia/quick-xml/issues/719>
    //
    // This is a temporary solution, we should find a better way to handle this.
    let s = s.replace("&()_+-=;", "%26%28%29_%2B-%3D%3B");

    quick_xml::de::from_str(&s).map_err(new_xml_deserialize_error)
}

pub fn parse_propstat(propstat: &Propstat) -> Result<Metadata> {
    let Propstat {
        prop:
            Prop {
                getlastmodified,
                getcontentlength,
                getcontenttype,
                getetag,
                resourcetype,
                ..
            },
        status,
    } = propstat;

    let mut status_parts = status.split_whitespace();
    status_parts
        .next()
        .ok_or_else(|| Error::new(ErrorKind::Unexpected, "propfind response status is missing"))?;
    let code = status_parts.next().ok_or_else(|| {
        Error::new(
            ErrorKind::Unexpected,
            "propfind response status code is missing",
        )
    })?;

    // read status definition at https://tools.ietf.org/html/rfc2068#section-6.1
    let status_parts = status.split_whitespace();
    let code = code.parse::<u16>().map_err(|err| {
        Error::new(ErrorKind::Unexpected, "parse webdav propfind status code").set_source(err)
    })?;
    if code >= 400 {
        let text = status_parts.collect::<Vec<_>>().join(" ");
        return Err(Error::new(
            ErrorKind::Unexpected,
            format!("propfind response is unexpected: {code} {text}"),
        ));
    }

    let mode: EntryMode = if resourcetype.value == Some(ResourceType::Collection) {
        EntryMode::DIR
    } else {
        EntryMode::FILE
    };
    let mut m = if mode == EntryMode::FILE {
        let content_length = getcontentlength.as_ref().ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "webdav response does not contain file content length",
            )
        })?;
        MetadataBuilder::file(content_length.parse::<u64>().map_err(|err| {
            Error::new(ErrorKind::Unexpected, "parse webdav content length").set_source(err)
        })?)
    } else {
        MetadataBuilder::dir()
    };

    if let Some(v) = getcontenttype {
        m.content_type(v);
    }

    if let Some(v) = getetag {
        m.etag(v);
    }

    // https://www.rfc-editor.org/rfc/rfc4918#section-14.18
    let Some(getlastmodified) = getlastmodified else {
        return Err(Error::new(
            ErrorKind::Unexpected,
            "propfind response missing getlastmodified",
        ));
    };
    m.last_modified(Timestamp::parse_rfc2822(getlastmodified)?);

    // the storage services have returned all the properties
    Ok(m.build())
}

pub fn parse_propstats(propstats: &[Propstat]) -> Result<Metadata> {
    let mut last_error = None;

    for propstat in propstats {
        match parse_propstat(propstat) {
            Ok(metadata) => return Ok(metadata),
            Err(err) => last_error = Some(err),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        Error::new(
            ErrorKind::Unexpected,
            "propfind response does not contain propstat",
        )
    }))
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Default)]
#[serde(default)]
pub struct Multistatus {
    pub response: Vec<PropfindResponse>,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct PropfindResponse {
    pub href: String,
    #[serde(default)]
    pub propstat: Vec<Propstat>,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Propstat {
    pub status: String,
    pub prop: Prop,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Prop {
    pub getlastmodified: Option<String>,
    pub getetag: Option<String>,
    pub getcontentlength: Option<String>,
    pub getcontenttype: Option<String>,
    #[serde(default)]
    pub resourcetype: ResourceTypeContainer,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Default)]
pub struct ResourceTypeContainer {
    #[serde(rename = "$value")]
    pub value: Option<ResourceType>,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    Collection,
}

#[cfg(test)]
mod tests {
    use quick_xml::de::from_str;

    use super::*;

    fn new_propstat(status: &str, getcontentlength: Option<&str>) -> Propstat {
        Propstat {
            status: status.to_string(),
            prop: Prop {
                getlastmodified: Some("Sun, 01 May 2022 06:39:47 GMT".to_string()),
                getetag: None,
                getcontentlength: getcontentlength.map(str::to_string),
                getcontenttype: None,
                resourcetype: ResourceTypeContainer { value: None },
            },
        }
    }

    #[test]
    fn test_propstat() {
        let xml = r#"<D:propstat>
            <D:prop>
                <D:displayname>/</D:displayname>
                <D:getlastmodified>Tue, 01 May 2022 06:39:47 GMT</D:getlastmodified>
                <D:resourcetype><D:collection/></D:resourcetype>
                <D:lockdiscovery/>
                <D:supportedlock>
                    <D:lockentry>
                        <D:lockscope><D:exclusive/></D:lockscope>
                        <D:locktype><D:write/></D:locktype>
                    </D:lockentry>
                </D:supportedlock>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
        </D:propstat>"#;

        let propstat = from_str::<Propstat>(xml).unwrap();
        assert_eq!(
            propstat.prop.getlastmodified.as_deref(),
            Some("Tue, 01 May 2022 06:39:47 GMT")
        );
        assert_eq!(
            propstat.prop.resourcetype.value,
            Some(ResourceType::Collection)
        );

        assert_eq!(propstat.status, "HTTP/1.1 200 OK");
    }

    #[test]
    fn test_parse_propstat_rejects_invalid_status_code() {
        for status in ["HTTP/1.1 abc OK", "HTTP/1.1 99999 OK", "HTTP/1.1", "broken"] {
            assert!(parse_propstat(&new_propstat(status, None)).is_err());
        }
    }

    #[test]
    fn test_parse_propstat_rejects_invalid_content_length() {
        for content_length in ["abc", "18446744073709551616"] {
            assert!(
                parse_propstat(&new_propstat("HTTP/1.1 200 OK", Some(content_length))).is_err()
            );
        }
    }

    #[test]
    fn test_response_simple() {
        let xml = r#"<D:response>
            <D:href>/</D:href>
            <D:propstat>
                <D:prop>
                    <D:displayname>/</D:displayname>
                    <D:getlastmodified>Tue, 01 May 2022 06:39:47 GMT</D:getlastmodified>
                    <D:resourcetype><D:collection/></D:resourcetype>
                    <D:lockdiscovery/>
                    <D:supportedlock>
                        <D:lockentry>
                            <D:lockscope><D:exclusive/></D:lockscope>
                            <D:locktype><D:write/></D:locktype>
                        </D:lockentry>
                    </D:supportedlock>
                </D:prop>
                <D:status>HTTP/1.1 200 OK</D:status>
            </D:propstat>
        </D:response>"#;

        let response = from_str::<PropfindResponse>(xml).unwrap();
        assert_eq!(response.href, "/");
        assert_eq!(response.propstat.len(), 1);

        assert_eq!(
            response.propstat[0].prop.getlastmodified.as_deref(),
            Some("Tue, 01 May 2022 06:39:47 GMT")
        );
        assert_eq!(
            response.propstat[0].prop.resourcetype.value,
            Some(ResourceType::Collection)
        );
        assert_eq!(response.propstat[0].status, "HTTP/1.1 200 OK");
    }

    #[test]
    fn test_response_file() {
        let xml = r#"<D:response>
        <D:href>/test_file</D:href>
        <D:propstat>
          <D:prop>
            <D:displayname>test_file</D:displayname>
            <D:getcontentlength>1</D:getcontentlength>
            <D:getlastmodified>Tue, 07 May 2022 05:52:22 GMT</D:getlastmodified>
            <D:resourcetype></D:resourcetype>
            <D:lockdiscovery />
            <D:supportedlock>
              <D:lockentry>
                <D:lockscope>
                  <D:exclusive />
                </D:lockscope>
                <D:locktype>
                  <D:write />
                </D:locktype>
              </D:lockentry>
            </D:supportedlock>
          </D:prop>
          <D:status>HTTP/1.1 200 OK</D:status>
        </D:propstat>
      </D:response>"#;

        let response = from_str::<PropfindResponse>(xml).unwrap();
        assert_eq!(response.href, "/test_file");
        assert_eq!(
            response.propstat[0].prop.getlastmodified.as_deref(),
            Some("Tue, 07 May 2022 05:52:22 GMT")
        );
        assert_eq!(
            response.propstat[0].prop.getcontentlength.as_deref(),
            Some("1")
        );
        assert_eq!(response.propstat[0].prop.resourcetype.value, None);
        assert_eq!(response.propstat[0].status, "HTTP/1.1 200 OK");
    }

    #[test]
    fn test_response_with_multiple_propstat() {
        let xml = r#"<D:response>
                    <D:href>/webdav/example/</D:href>
                    <D:propstat>
                        <D:prop>
                            <D:resourcetype><D:collection/></D:resourcetype>
                            <D:displayname>example-folder</D:displayname>
                            <D:getlastmodified>Thu, 25 Jun 2026 06:23:30 GMT</D:getlastmodified>
                            <D:getetag>"example-etag"</D:getetag>
                        </D:prop>
                        <D:status>HTTP/1.1 200 OK</D:status>
                    </D:propstat>
                    <D:propstat>
                        <D:prop>
                            <D:creationdate/>
                            <D:getcontentlength/>
                            <D:getcontenttype/>
                            <D:getcontentlanguage/>
                            <D:source/>
                        </D:prop>
                        <D:status>HTTP/1.1 404 Not Found</D:status>
                    </D:propstat>
                </D:response>"#;

        let response = from_str::<PropfindResponse>(xml).unwrap();
        assert_eq!(response.propstat.len(), 2);

        let meta = parse_propstats(&response.propstat).unwrap();
        assert!(meta.is_dir());
        assert_eq!(meta.etag(), Some("\"example-etag\""));
    }

    #[test]
    fn test_parse_propstats_skips_failed_propstat() {
        let propstats = vec![
            Propstat {
                status: "HTTP/1.1 404 Not Found".to_string(),
                prop: Prop {
                    getlastmodified: None,
                    getetag: None,
                    getcontentlength: None,
                    getcontenttype: None,
                    resourcetype: ResourceTypeContainer::default(),
                },
            },
            new_propstat("HTTP/1.1 200 OK", Some("0")),
        ];

        let meta = parse_propstats(&propstats).unwrap();
        assert!(meta.is_file());
        assert_eq!(meta.content_length(), 0);
    }

    #[test]
    fn test_with_multiple_items_simple() {
        let xml = r#"<D:multistatus xmlns:D="DAV:">
        <D:response>
        <D:href>/</D:href>
        <D:propstat>
            <D:prop>
                <D:displayname>/</D:displayname>
                <D:getlastmodified>Tue, 01 May 2022 06:39:47 GMT</D:getlastmodified>
                <D:resourcetype><D:collection/></D:resourcetype>
                <D:lockdiscovery/>
                <D:supportedlock>
                    <D:lockentry>
                        <D:lockscope><D:exclusive/></D:lockscope>
                        <D:locktype><D:write/></D:locktype>
                    </D:lockentry>
                </D:supportedlock>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
        </D:propstat>
    </D:response>
    <D:response>
            <D:href>/</D:href>
            <D:propstat>
                <D:prop>
                    <D:displayname>/</D:displayname>
                    <D:getlastmodified>Tue, 01 May 2022 06:39:47 GMT</D:getlastmodified>
                    <D:resourcetype><D:collection/></D:resourcetype>
                    <D:lockdiscovery/>
                    <D:supportedlock>
                        <D:lockentry>
                            <D:lockscope><D:exclusive/></D:lockscope>
                            <D:locktype><D:write/></D:locktype>
                        </D:lockentry>
                    </D:supportedlock>
                </D:prop>
                <D:status>HTTP/1.1 200 OK</D:status>
            </D:propstat>
        </D:response>
        </D:multistatus>"#;

        let multistatus = from_str::<Multistatus>(xml).unwrap();

        let response = multistatus.response;
        assert_eq!(response.len(), 2);
        assert_eq!(response[0].href, "/");
        assert_eq!(
            response[0].propstat[0].prop.getlastmodified.as_deref(),
            Some("Tue, 01 May 2022 06:39:47 GMT")
        );
    }

    #[test]
    fn test_with_multiple_items_mixed() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <D:multistatus xmlns:D="DAV:">
          <D:response>
            <D:href>/</D:href>
            <D:propstat>
              <D:prop>
                <D:displayname>/</D:displayname>
                <D:getlastmodified>Tue, 07 May 2022 06:39:47 GMT</D:getlastmodified>
                <D:resourcetype>
                  <D:collection />
                </D:resourcetype>
                <D:lockdiscovery />
                <D:supportedlock>
                  <D:lockentry>
                    <D:lockscope>
                      <D:exclusive />
                    </D:lockscope>
                    <D:locktype>
                      <D:write />
                    </D:locktype>
                  </D:lockentry>
                </D:supportedlock>
              </D:prop>
              <D:status>HTTP/1.1 200 OK</D:status>
            </D:propstat>
          </D:response>
          <D:response>
            <D:href>/testdir/</D:href>
            <D:propstat>
              <D:prop>
                <D:displayname>testdir</D:displayname>
                <D:getlastmodified>Tue, 07 May 2022 06:40:10 GMT</D:getlastmodified>
                <D:resourcetype>
                  <D:collection />
                </D:resourcetype>
                <D:lockdiscovery />
                <D:supportedlock>
                  <D:lockentry>
                    <D:lockscope>
                      <D:exclusive />
                    </D:lockscope>
                    <D:locktype>
                      <D:write />
                    </D:locktype>
                  </D:lockentry>
                </D:supportedlock>
              </D:prop>
              <D:status>HTTP/1.1 200 OK</D:status>
            </D:propstat>
          </D:response>
          <D:response>
            <D:href>/test_file</D:href>
            <D:propstat>
              <D:prop>
                <D:displayname>test_file</D:displayname>
                <D:getcontentlength>1</D:getcontentlength>
                <D:getlastmodified>Tue, 07 May 2022 05:52:22 GMT</D:getlastmodified>
                <D:resourcetype></D:resourcetype>
                <D:lockdiscovery />
                <D:supportedlock>
                  <D:lockentry>
                    <D:lockscope>
                      <D:exclusive />
                    </D:lockscope>
                    <D:locktype>
                      <D:write />
                    </D:locktype>
                  </D:lockentry>
                </D:supportedlock>
              </D:prop>
              <D:status>HTTP/1.1 200 OK</D:status>
            </D:propstat>
          </D:response>
        </D:multistatus>"#;

        let multistatus = from_str::<Multistatus>(xml).unwrap();

        let response = multistatus.response;
        assert_eq!(response.len(), 3);
        let first_response = &response[0];
        assert_eq!(first_response.href, "/");
        assert_eq!(
            first_response.propstat[0].prop.getlastmodified.as_deref(),
            Some("Tue, 07 May 2022 06:39:47 GMT")
        );

        let second_response = &response[1];
        assert_eq!(second_response.href, "/testdir/");
        assert_eq!(
            second_response.propstat[0].prop.getlastmodified.as_deref(),
            Some("Tue, 07 May 2022 06:40:10 GMT")
        );

        let third_response = &response[2];
        assert_eq!(third_response.href, "/test_file");
        assert_eq!(
            third_response.propstat[0].prop.getlastmodified.as_deref(),
            Some("Tue, 07 May 2022 05:52:22 GMT")
        );
    }

    #[test]
    fn test_with_multiple_items_mixed_nginx() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
      <D:multistatus xmlns:D="DAV:">
        <D:response>
          <D:href>/</D:href>
          <D:propstat>
            <D:prop>
              <D:getlastmodified>Fri, 17 Feb 2023 03:37:22 GMT</D:getlastmodified>
              <D:resourcetype>
                <D:collection />
              </D:resourcetype>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
          </D:propstat>
        </D:response>
        <D:response>
          <D:href>/test_file_75</D:href>
          <D:propstat>
            <D:prop>
              <D:getcontentlength>1</D:getcontentlength>
              <D:getlastmodified>Fri, 17 Feb 2023 03:36:54 GMT</D:getlastmodified>
              <D:resourcetype></D:resourcetype>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
          </D:propstat>
        </D:response>
        <D:response>
          <D:href>/test_file_36</D:href>
          <D:propstat>
            <D:prop>
              <D:getcontentlength>1</D:getcontentlength>
              <D:getlastmodified>Fri, 17 Feb 2023 03:36:54 GMT</D:getlastmodified>
              <D:resourcetype></D:resourcetype>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
          </D:propstat>
        </D:response>
        <D:response>
          <D:href>/test_file_38</D:href>
          <D:propstat>
            <D:prop>
              <D:getcontentlength>1</D:getcontentlength>
              <D:getlastmodified>Fri, 17 Feb 2023 03:36:54 GMT</D:getlastmodified>
              <D:resourcetype></D:resourcetype>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
          </D:propstat>
        </D:response>
        <D:response>
          <D:href>/test_file_59</D:href>
          <D:propstat>
            <D:prop>
              <D:getcontentlength>1</D:getcontentlength>
              <D:getlastmodified>Fri, 17 Feb 2023 03:36:54 GMT</D:getlastmodified>
              <D:resourcetype></D:resourcetype>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
          </D:propstat>
        </D:response>
        <D:response>
          <D:href>/test_file_9</D:href>
          <D:propstat>
            <D:prop>
              <D:getcontentlength>1</D:getcontentlength>
              <D:getlastmodified>Fri, 17 Feb 2023 03:36:54 GMT</D:getlastmodified>
              <D:resourcetype></D:resourcetype>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
          </D:propstat>
        </D:response>
        <D:response>
          <D:href>/test_file_93</D:href>
          <D:propstat>
            <D:prop>
              <D:getcontentlength>1</D:getcontentlength>
              <D:getlastmodified>Fri, 17 Feb 2023 03:36:54 GMT</D:getlastmodified>
              <D:resourcetype></D:resourcetype>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
          </D:propstat>
        </D:response>
        <D:response>
          <D:href>/test_file_43</D:href>
          <D:propstat>
            <D:prop>
              <D:getcontentlength>1</D:getcontentlength>
              <D:getlastmodified>Fri, 17 Feb 2023 03:36:54 GMT</D:getlastmodified>
              <D:resourcetype></D:resourcetype>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
          </D:propstat>
        </D:response>
        <D:response>
          <D:href>/test_file_95</D:href>
          <D:propstat>
            <D:prop>
              <D:getcontentlength>1</D:getcontentlength>
              <D:getlastmodified>Fri, 17 Feb 2023 03:36:54 GMT</D:getlastmodified>
              <D:resourcetype></D:resourcetype>
            </D:prop>
            <D:status>HTTP/1.1 200 OK</D:status>
          </D:propstat>
        </D:response>
      </D:multistatus>
      "#;

        let multistatus: Multistatus = from_str(xml).unwrap();

        let response = multistatus.response;
        assert_eq!(response.len(), 9);

        let first_response = &response[0];
        assert_eq!(first_response.href, "/");
        assert_eq!(
            first_response.propstat[0].prop.getlastmodified.as_deref(),
            Some("Fri, 17 Feb 2023 03:37:22 GMT")
        );
    }

    #[test]
    fn test_build_proppatch_request() {
        let mut user_metadata = HashMap::new();
        user_metadata.insert("key1".to_string(), "value1".to_string());
        user_metadata.insert("key2".to_string(), "value2".to_string());

        let request = build_proppatch_request(
            &user_metadata,
            DEFAULT_USER_METADATA_PREFIX,
            DEFAULT_USER_METADATA_URI,
        );

        // Check that the request contains the expected XML structure
        assert!(request.contains(r#"<?xml version="1.0" encoding="utf-8"?>"#));
        assert!(request.contains(r#"<D:propertyupdate"#));
        assert!(request.contains(r#"xmlns:D="DAV:""#));
        assert!(request.contains(&format!(
            r#"xmlns:{DEFAULT_USER_METADATA_PREFIX}="{DEFAULT_USER_METADATA_URI}""#
        )));
        assert!(request.contains(r#"<D:set>"#));
        assert!(request.contains(r#"<D:prop>"#));
        // Check that user metadata is included (order may vary)
        assert!(request.contains(&format!(
            "<{DEFAULT_USER_METADATA_PREFIX}:key1>value1</{DEFAULT_USER_METADATA_PREFIX}:key1>"
        )));
        assert!(request.contains(&format!(
            "<{DEFAULT_USER_METADATA_PREFIX}:key2>value2</{DEFAULT_USER_METADATA_PREFIX}:key2>"
        )));
    }

    #[test]
    fn test_build_proppatch_request_with_special_chars() {
        let mut user_metadata = HashMap::new();
        user_metadata.insert("key".to_string(), "value<>&\"'".to_string());

        let request = build_proppatch_request(
            &user_metadata,
            DEFAULT_USER_METADATA_PREFIX,
            DEFAULT_USER_METADATA_URI,
        );

        // Check that special characters are properly escaped
        assert!(request.contains("value&lt;&gt;&amp;&quot;&apos;"));
    }

    #[test]
    fn test_build_proppatch_request_custom_namespace() {
        let mut user_metadata = HashMap::new();
        user_metadata.insert("key1".to_string(), "value1".to_string());

        let request = build_proppatch_request(&user_metadata, "custom", "http://example.com/ns");

        // Check that custom namespace is used
        assert!(request.contains(r#"xmlns:custom="http://example.com/ns""#));
        assert!(request.contains("<custom:key1>value1</custom:key1>"));
    }

    #[test]
    fn test_parse_user_metadata_from_xml() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <D:multistatus xmlns:D="DAV:" xmlns:opendal="https://opendal.apache.org/ns">
          <D:response>
            <D:propstat>
              <D:prop>
                <D:getlastmodified>Fri, 17 Feb 2023 03:37:22 GMT</D:getlastmodified>
                <opendal:key1>value1</opendal:key1>
                <opendal:key2>value2</opendal:key2>
              </D:prop>
              <D:status>HTTP/1.1 200 OK</D:status>
            </D:propstat>
          </D:response>
        </D:multistatus>"#;

        let user_metadata = parse_user_metadata_from_xml(xml, DEFAULT_USER_METADATA_URI);

        assert_eq!(user_metadata.len(), 2);
        assert_eq!(user_metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(user_metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_parse_user_metadata_from_xml_with_special_chars() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <D:multistatus xmlns:D="DAV:" xmlns:opendal="https://opendal.apache.org/ns">
          <D:response>
            <D:propstat>
              <D:prop>
                <opendal:key>value&lt;&gt;&amp;&quot;&apos;</opendal:key>
              </D:prop>
              <D:status>HTTP/1.1 200 OK</D:status>
            </D:propstat>
          </D:response>
        </D:multistatus>"#;

        let user_metadata = parse_user_metadata_from_xml(xml, DEFAULT_USER_METADATA_URI);

        assert_eq!(user_metadata.len(), 1);
        assert_eq!(user_metadata.get("key"), Some(&"value<>&\"'".to_string()));
    }

    #[test]
    fn test_parse_user_metadata_from_xml_empty() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <D:multistatus xmlns:D="DAV:">
          <D:response>
            <D:propstat>
              <D:prop>
                <D:getlastmodified>Fri, 17 Feb 2023 03:37:22 GMT</D:getlastmodified>
              </D:prop>
              <D:status>HTTP/1.1 200 OK</D:status>
            </D:propstat>
          </D:response>
        </D:multistatus>"#;

        let user_metadata = parse_user_metadata_from_xml(xml, DEFAULT_USER_METADATA_URI);

        assert!(user_metadata.is_empty());
    }

    #[test]
    fn test_parse_user_metadata_from_xml_dynamic_prefix() {
        // Nextcloud/ownCloud style: uses dynamic namespace prefixes (x1:, x2:, etc.)
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <d:multistatus xmlns:d="DAV:">
          <d:response>
            <d:propstat>
              <d:prop>
                <d:getlastmodified>Fri, 17 Feb 2023 03:37:22 GMT</d:getlastmodified>
                <x1:key1 xmlns:x1="https://opendal.apache.org/ns">value1</x1:key1>
                <x2:key2 xmlns:x2="https://opendal.apache.org/ns">value2</x2:key2>
              </d:prop>
              <d:status>HTTP/1.1 200 OK</d:status>
            </d:propstat>
          </d:response>
        </d:multistatus>"#;

        let user_metadata = parse_user_metadata_from_xml(xml, DEFAULT_USER_METADATA_URI);

        assert_eq!(user_metadata.len(), 2);
        assert_eq!(user_metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(user_metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_parse_user_metadata_from_xml_namespace_at_root() {
        // Alternative: namespace declared at root level
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <d:multistatus xmlns:d="DAV:" xmlns:custom="https://opendal.apache.org/ns">
          <d:response>
            <d:propstat>
              <d:prop>
                <custom:location>everywhere</custom:location>
              </d:prop>
              <d:status>HTTP/1.1 200 OK</d:status>
            </d:propstat>
          </d:response>
        </d:multistatus>"#;

        let user_metadata = parse_user_metadata_from_xml(xml, DEFAULT_USER_METADATA_URI);

        assert_eq!(user_metadata.len(), 1);
        assert_eq!(
            user_metadata.get("location"),
            Some(&"everywhere".to_string())
        );
    }

    #[test]
    fn test_parse_user_metadata_from_xml_custom_namespace() {
        // Test with a custom namespace (like what users might configure)
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <d:multistatus xmlns:d="DAV:" xmlns:myapp="http://myapp.example.com/ns">
          <d:response>
            <d:propstat>
              <d:prop>
                <myapp:author>John Doe</myapp:author>
              </d:prop>
              <d:status>HTTP/1.1 200 OK</d:status>
            </d:propstat>
          </d:response>
        </d:multistatus>"#;

        let user_metadata = parse_user_metadata_from_xml(xml, "http://myapp.example.com/ns");

        assert_eq!(user_metadata.len(), 1);
        assert_eq!(user_metadata.get("author"), Some(&"John Doe".to_string()));
    }

    #[test]
    fn test_check_proppatch_response_success() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <D:multistatus xmlns:D="DAV:" xmlns:opendal="https://opendal.apache.org/ns">
          <D:response>
            <D:href>/test.txt</D:href>
            <D:propstat>
              <D:prop>
                <opendal:location/>
              </D:prop>
              <D:status>HTTP/1.1 200 OK</D:status>
            </D:propstat>
          </D:response>
        </D:multistatus>"#;

        assert!(check_proppatch_response(xml).is_ok());
    }

    #[test]
    fn test_check_proppatch_response_failure() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <D:multistatus xmlns:D="DAV:" xmlns:opendal="https://opendal.apache.org/ns">
          <D:response>
            <D:href>/test.txt</D:href>
            <D:propstat>
              <D:prop>
                <opendal:location/>
              </D:prop>
              <D:status>HTTP/1.1 403 Forbidden</D:status>
            </D:propstat>
          </D:response>
        </D:multistatus>"#;

        assert!(check_proppatch_response(xml).is_err());
    }

    #[test]
    fn test_check_proppatch_response_lowercase() {
        // Nextcloud might use lowercase "d:" prefix
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
        <d:multistatus xmlns:d="DAV:">
          <d:response>
            <d:href>/test.txt</d:href>
            <d:propstat>
              <d:prop/>
              <d:status>HTTP/1.1 200 OK</d:status>
            </d:propstat>
          </d:response>
        </d:multistatus>"#;

        assert!(check_proppatch_response(xml).is_ok());
    }

    #[test]
    fn test_parse_error_maps_missing_if_match_to_not_found() {
        let resp = Response::builder()
            .status(StatusCode::PRECONDITION_FAILED)
            .body(Buffer::from(
                "An If-Match header was specified and the resource did not exist",
            ))
            .unwrap();

        let err = parse_error(
            ErrorContext::new(ServiceOperation("Get")).with_if_match(true),
            resp,
        );
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn test_parse_error_preserves_etag_mismatch() {
        let resp = Response::builder()
            .status(StatusCode::PRECONDITION_FAILED)
            .body(Buffer::from("The ETag did not match"))
            .unwrap();

        let err = parse_error(
            ErrorContext::new(ServiceOperation("Get")).with_if_match(true),
            resp,
        );
        assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    }
}

/// Context needed to classify an error from this service.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ErrorContext {
    service_operation: ServiceOperation,
    if_match: bool,
}

impl ErrorContext {
    pub(crate) const fn new(service_operation: ServiceOperation) -> Self {
        Self {
            service_operation,
            if_match: false,
        }
    }

    pub(crate) const fn with_if_match(mut self, if_match: bool) -> Self {
        self.if_match = if_match;
        self
    }
}

/// Parse an error response using its service request context.
pub(crate) fn parse_error(ctx: ErrorContext, resp: Response<Buffer>) -> Error {
    let (parts, body) = resp.into_parts();
    let bs = body.to_bytes();

    let message = String::from_utf8_lossy(&bs);

    let (kind, retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        // Some services (like owncloud) return 403 while file locked.
        StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, true),
        // RFC 7232: 412 means an If-Match / If-Unmodified-Since
        // precondition failed; 304 means an If-None-Match /
        // If-Modified-Since precondition matched. Surface both as
        // ConditionNotMatch so callers can branch on it.
        StatusCode::PRECONDITION_FAILED
            if ctx.if_match && message.contains("resource did not exist") =>
        {
            (ErrorKind::NotFound, false)
        }
        StatusCode::PRECONDITION_FAILED | StatusCode::NOT_MODIFIED => {
            (ErrorKind::ConditionNotMatch, false)
        }
        // Allowing retry for resource locked.
        StatusCode::LOCKED => (ErrorKind::Unexpected, true),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}
