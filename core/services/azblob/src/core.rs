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

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use bytes::Buf;
use bytes::Bytes;
use constants::X_MS_META_PREFIX;
use http::HeaderValue;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::header::HeaderName;
use http::header::IF_MATCH;
use http::header::IF_MODIFIED_SINCE;
use http::header::IF_NONE_MATCH;
use http::header::IF_UNMODIFIED_SINCE;
use opendal_service_azure_common::with_azure_error_response_context;
use quick_xml::de;
use reqsign_azure_storage::Credential;
use reqsign_core::{Context, Signer};
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use opendal_core::raw::*;
use opendal_core::*;

pub mod constants {
    pub const AZBLOB_COPY_MIN_BLOCK_SIZE: usize = 1;
    // Put Block From URL supports blocks up to 4,000 MiB with API version
    // 2020-04-08 and later.
    //
    // ref: <https://learn.microsoft.com/en-us/rest/api/storageservices/put-block-from-url>
    pub const AZBLOB_COPY_MAX_BLOCK_SIZE: usize = 4000 * 1024 * 1024;

    // Indicates the Blob Storage version that was used to execute the request
    pub const X_MS_VERSION: &str = "x-ms-version";

    pub const X_MS_BLOB_TYPE: &str = "x-ms-blob-type";
    pub const X_MS_COPY_SOURCE: &str = "x-ms-copy-source";
    pub const X_MS_COPY_SOURCE_RANGE: &str = "x-ms-source-range";
    pub const X_MS_BLOB_CACHE_CONTROL: &str = "x-ms-blob-cache-control";
    pub const X_MS_BLOB_CONTENT_TYPE: &str = "x-ms-blob-content-type";
    pub const X_MS_BLOB_CONDITION_APPENDPOS: &str = "x-ms-blob-condition-appendpos";
    pub const X_MS_META_PREFIX: &str = "x-ms-meta-";

    // indicates the version of the blob, and it can be used in subsequent requests to access the blob.
    pub const X_MS_VERSION_ID: &str = "x-ms-version-id";

    // Server-side encryption with customer-provided headers
    pub const X_MS_ENCRYPTION_KEY: &str = "x-ms-encryption-key";
    pub const X_MS_ENCRYPTION_KEY_SHA256: &str = "x-ms-encryption-key-sha256";
    pub const X_MS_ENCRYPTION_ALGORITHM: &str = "x-ms-encryption-algorithm";
}

pub struct AzblobCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub container: String,
    pub root: String,
    pub endpoint: String,
    pub encryption_key: Option<HeaderValue>,
    pub encryption_key_sha256: Option<HeaderValue>,
    pub encryption_algorithm: Option<HeaderValue>,
    pub skip_signature: bool,
    pub signer: Signer<Credential>,
}

impl Debug for AzblobCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzblobCore")
            .field("container", &self.container)
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl AzblobCore {
    fn signer(&self, ctx: &OperationContext) -> Signer<Credential> {
        self.signer.clone().with_context(
            Context::new()
                .with_file_read(reqsign_file_read_tokio::TokioFileRead)
                .with_http_send(ctx.http_transport().clone())
                .with_env(reqsign_core::OsEnv),
        )
    }

    pub async fn sign_query<T>(
        &self,
        ctx: &OperationContext,
        req: Request<T>,
    ) -> Result<Request<T>> {
        if self.skip_signature {
            return Ok(req);
        }

        let (mut parts, body) = req.into_parts();

        self.signer(ctx)
            .sign(&mut parts, Some(Duration::from_secs(3600)))
            .await
            .map_err(|e| new_request_sign_error(e.into()))?;

        Ok(Request::from_parts(parts, body))
    }

    pub async fn sign<T>(&self, ctx: &OperationContext, req: Request<T>) -> Result<Request<T>> {
        let (mut parts, body) = req.into_parts();

        // Insert x-ms-version header for normal requests.
        parts.headers.insert(
            HeaderName::from_static(constants::X_MS_VERSION),
            // 2022-11-02 is the version supported by Azurite V3 and
            // used by Azure Portal, We use this version to make
            // sure most our developer happy.
            //
            // In the future, we could allow users to configure this value.
            HeaderValue::from_static("2022-11-02"),
        );

        // x-ms-version must be set even when signing is skipped.
        if self.skip_signature {
            return Ok(Request::from_parts(parts, body));
        }

        self.signer(ctx)
            .sign(&mut parts, None)
            .await
            .map_err(|e| new_request_sign_error(e.into()))?;

        Ok(Request::from_parts(parts, body))
    }

    async fn batch_sign<T>(&self, ctx: &OperationContext, req: Request<T>) -> Result<Request<T>> {
        if self.skip_signature {
            return Ok(req);
        }

        let (mut parts, body) = req.into_parts();

        self.signer(ctx)
            .sign(&mut parts, None)
            .await
            .map_err(|e| new_request_sign_error(e.into()))?;

        Ok(Request::from_parts(parts, body))
    }

    #[inline]
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    pub fn insert_sse_headers(&self, mut req: http::request::Builder) -> http::request::Builder {
        if let Some(v) = &self.encryption_key {
            let mut v = v.clone();
            v.set_sensitive(true);

            req = req.header(HeaderName::from_static(constants::X_MS_ENCRYPTION_KEY), v)
        }

        if let Some(v) = &self.encryption_key_sha256 {
            let mut v = v.clone();
            v.set_sensitive(true);

            req = req.header(
                HeaderName::from_static(constants::X_MS_ENCRYPTION_KEY_SHA256),
                v,
            )
        }

        if let Some(v) = &self.encryption_algorithm {
            let mut v = v.clone();
            v.set_sensitive(true);

            req = req.header(
                HeaderName::from_static(constants::X_MS_ENCRYPTION_ALGORITHM),
                v,
            )
        }

        req
    }
}

impl AzblobCore {
    fn build_path_url(&self, path: &str) -> String {
        format!(
            "{}/{}/{}",
            self.endpoint,
            self.container,
            percent_encode_path(&build_abs_path(&self.root, path))
        )
    }

    pub fn azblob_get_blob_request(
        &self,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Request<Buffer>> {
        let mut url = self.build_path_url(path);

        if let Some(override_content_disposition) = args.override_content_disposition() {
            url.push_str(&format!(
                "?rscd={}",
                percent_encode_path(override_content_disposition)
            ));
        }

        let mut req = Request::get(&url);

        // Set SSE headers.
        req = self.insert_sse_headers(req);

        if !range.is_full() {
            req = req.header(http::header::RANGE, range.to_header());
        }

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }

        if let Some(if_modified_since) = args.if_modified_since() {
            req = req.header(IF_MODIFIED_SINCE, if_modified_since.format_http_date());
        }

        if let Some(if_unmodified_since) = args.if_unmodified_since() {
            req = req.header(IF_UNMODIFIED_SINCE, if_unmodified_since.format_http_date());
        }

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("GetBlob"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn azblob_get_blob(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Response<HttpBody>> {
        let req = self.azblob_get_blob_request(path, range, args)?;
        let req = self.sign(ctx, req).await?;

        ctx.http_transport().fetch(req).await
    }

    pub fn azblob_put_blob_request(
        &self,
        path: &str,
        size: Option<u64>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let mut req = Request::put(self.build_path_url(path));

        req = req.header(
            HeaderName::from_static(constants::X_MS_BLOB_TYPE),
            "BlockBlob",
        );

        if let Some(size) = size {
            req = req.header(CONTENT_LENGTH, size)
        }

        if let Some(ty) = args.content_type() {
            req = req.header(CONTENT_TYPE, ty)
        }

        // Specify the wildcard character (*) to perform the operation only if
        // the resource does not exist, and fail the operation if it does exist.
        if args.if_not_exists() {
            req = req.header(IF_NONE_MATCH, "*");
        }
        if let Some(v) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, v);
        }

        if let Some(v) = args.if_match() {
            req = req.header(IF_MATCH, v);
        }

        if let Some(cache_control) = args.cache_control() {
            req = req.header(constants::X_MS_BLOB_CACHE_CONTROL, cache_control);
        }

        // Set SSE headers.
        req = self.insert_sse_headers(req);

        if let Some(user_metadata) = args.user_metadata() {
            for (key, value) in user_metadata {
                req = req.header(format!("{X_MS_META_PREFIX}{key}"), value)
            }
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("PutBlob"))
            .body(body)
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn azblob_put_blob(
        &self,
        ctx: &OperationContext,
        path: &str,
        size: Option<u64>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let req = self.azblob_put_blob_request(path, size, args, body)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    /// For appendable object, it could be created by `put` an empty blob
    /// with `x-ms-blob-type` header set to `AppendBlob`.
    /// And it's just initialized with empty content.
    ///
    /// If want to append content to it, we should use the following method `azblob_append_blob_request`.
    ///
    /// # Notes
    ///
    /// Appendable blob's custom header could only be set when it's created.
    ///
    /// The following custom header could be set:
    /// - `content-type`
    /// - `x-ms-blob-cache-control`
    ///
    /// # Reference
    ///
    /// https://learn.microsoft.com/en-us/rest/api/storageservices/put-blob
    fn azblob_init_appendable_blob_request(
        &self,
        path: &str,
        args: &OpWrite,
    ) -> Result<Request<Buffer>> {
        let mut req = Request::put(self.build_path_url(path));

        // Set SSE headers.
        req = self.insert_sse_headers(req);

        // The content-length header must be set to zero
        // when creating an appendable blob.
        req = req.header(CONTENT_LENGTH, 0);
        req = req.header(
            HeaderName::from_static(constants::X_MS_BLOB_TYPE),
            "AppendBlob",
        );

        if args.if_not_exists() {
            req = req.header(IF_NONE_MATCH, "*");
        }

        if let Some(ty) = args.content_type() {
            req = req.header(CONTENT_TYPE, ty)
        }

        if let Some(cache_control) = args.cache_control() {
            req = req.header(constants::X_MS_BLOB_CACHE_CONTROL, cache_control);
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("PutBlob"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn azblob_init_appendable_blob(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let req = self.azblob_init_appendable_blob_request(path, args)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    /// Append content to an appendable blob.
    /// The content will be appended to the end of the blob.
    ///
    /// # Notes
    ///
    /// - The maximum size of the content could be appended is 4MB.
    /// - `Append Block` succeeds only if the blob already exists.
    ///
    /// # Reference
    ///
    /// https://learn.microsoft.com/en-us/rest/api/storageservices/append-block
    fn azblob_append_blob_request(
        &self,
        path: &str,
        position: u64,
        size: u64,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let url = format!("{}?comp=appendblock", self.build_path_url(path));

        let mut req = Request::put(&url)
            .header(CONTENT_LENGTH, size)
            .header(constants::X_MS_BLOB_CONDITION_APPENDPOS, position);

        // Set SSE headers.
        req = self.insert_sse_headers(req);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("AppendBlock"))
            .body(body)
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn azblob_append_blob(
        &self,
        ctx: &OperationContext,
        path: &str,
        position: u64,
        size: u64,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let req = self.azblob_append_blob_request(path, position, size, body)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub fn azblob_put_block_request(
        &self,
        path: &str,
        block_id: Uuid,
        size: Option<u64>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        // To be written as part of a blob, a block must have been successfully written to the server in an earlier Put Block operation.
        // refer to https://learn.microsoft.com/en-us/rest/api/storageservices/put-block?tabs=microsoft-entra-id
        let url = QueryPairsWriter::new(&self.build_path_url(path))
            .push("comp", "block")
            .push(
                "blockid",
                &percent_encode_path(&BASE64_STANDARD.encode(block_id.as_bytes())),
            )
            .finish();

        let mut req = Request::put(&url);
        // Set SSE headers.
        req = self.insert_sse_headers(req);

        if let Some(cache_control) = args.cache_control() {
            req = req.header(constants::X_MS_BLOB_CACHE_CONTROL, cache_control);
        }
        if let Some(size) = size {
            req = req.header(CONTENT_LENGTH, size)
        }

        if let Some(ty) = args.content_type() {
            req = req.header(CONTENT_TYPE, ty)
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("PutBlock"))
            .body(body)
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn azblob_put_block(
        &self,
        ctx: &OperationContext,
        path: &str,
        block_id: Uuid,
        size: Option<u64>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let req = self.azblob_put_block_request(path, block_id, size, args, body)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub fn azblob_put_block_from_url_request(
        &self,
        source: &str,
        to: &str,
        block_id: Uuid,
        range: BytesRange,
    ) -> Result<Request<Buffer>> {
        let url = QueryPairsWriter::new(&self.build_path_url(to))
            .push("comp", "block")
            .push(
                "blockid",
                &percent_encode_path(&BASE64_STANDARD.encode(block_id.as_bytes())),
            )
            .finish();

        let mut req = Request::put(&url)
            .header(constants::X_MS_COPY_SOURCE, source)
            .header(CONTENT_LENGTH, 0);

        if !range.is_full() {
            req = req.header(constants::X_MS_COPY_SOURCE_RANGE, range.to_header());
        }

        let req = self.insert_sse_headers(req);
        let req = req
            .extension(Operation::Copy)
            .extension(ServiceOperation("PutBlockFromUrl"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn azblob_put_block_from_url(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        source_version: Option<&str>,
        block_id: Uuid,
        range: BytesRange,
    ) -> Result<Response<Buffer>> {
        let mut source_url = self.build_path_url(from);
        if let Some(version) = source_version {
            source_url = QueryPairsWriter::new(&source_url)
                .push("versionid", &percent_encode_path(version))
                .finish();
        }

        let source = Request::get(source_url)
            .extension(Operation::Copy)
            .extension(ServiceOperation("GetBlob"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        let source = self.sign_query(ctx, source).await?;
        let source = source.uri().to_string();
        let req = self.azblob_put_block_from_url_request(&source, to, block_id, range)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    fn azblob_complete_put_block_list_request(
        &self,
        path: &str,
        block_ids: Vec<Uuid>,
        args: &OpWrite,
    ) -> Result<Request<Buffer>> {
        let url = format!("{}?comp=blocklist", self.build_path_url(path));

        let req = Request::put(&url);

        // Set SSE headers.
        let mut req = self.insert_sse_headers(req);
        if let Some(cache_control) = args.cache_control() {
            req = req.header(constants::X_MS_BLOB_CACHE_CONTROL, cache_control);
        }

        // Put Block List is where Azure applies the blob's properties and metadata;
        // the headers on the individual Put Block requests are ignored.
        if let Some(ty) = args.content_type() {
            req = req.header(constants::X_MS_BLOB_CONTENT_TYPE, ty);
        }
        if let Some(user_metadata) = args.user_metadata() {
            for (key, value) in user_metadata {
                req = req.header(format!("{X_MS_META_PREFIX}{key}"), value);
            }
        }

        // Put Block List is the request that actually commits a blocked write, so the
        // write's preconditions have to be evaluated here rather than on Put Block.
        if args.if_not_exists() {
            req = req.header(IF_NONE_MATCH, "*");
        }

        if let Some(v) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, v);
        }

        if let Some(v) = args.if_match() {
            req = req.header(IF_MATCH, v);
        }

        let content = quick_xml::se::to_string(&PutBlockListRequest {
            latest: block_ids
                .into_iter()
                .map(|block_id| {
                    let encoded_block_id: String = BASE64_STANDARD.encode(block_id.as_bytes());
                    encoded_block_id
                })
                .collect(),
        })
        .map_err(new_xml_serialize_error)?;

        req = req.header(CONTENT_LENGTH, content.len());

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("PutBlockList"))
            .body(Buffer::from(Bytes::from(content)))
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn azblob_complete_put_block_list(
        &self,
        ctx: &OperationContext,
        path: &str,
        block_ids: Vec<Uuid>,
        args: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let req = self.azblob_complete_put_block_list_request(path, block_ids, args)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    fn azblob_complete_copy_block_list_request(
        &self,
        path: &str,
        block_ids: Vec<Uuid>,
        args: &OpCopy,
    ) -> Result<Request<Buffer>> {
        let url = format!("{}?comp=blocklist", self.build_path_url(path));

        let mut req = Request::put(&url);

        if args.if_not_exists() {
            req = req.header(IF_NONE_MATCH, "*");
        }
        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }
        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        let content = quick_xml::se::to_string(&PutBlockListRequest {
            latest: block_ids
                .into_iter()
                .map(|block_id| BASE64_STANDARD.encode(block_id.as_bytes()))
                .collect(),
        })
        .map_err(new_xml_serialize_error)?;

        req = req.header(CONTENT_LENGTH, content.len());

        let req = self.insert_sse_headers(req);
        let req = req
            .extension(Operation::Copy)
            .extension(ServiceOperation("PutBlockList"))
            .body(Buffer::from(Bytes::from(content)))
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn azblob_complete_copy_block_list(
        &self,
        ctx: &OperationContext,
        path: &str,
        block_ids: Vec<Uuid>,
        args: &OpCopy,
    ) -> Result<Response<Buffer>> {
        let req = self.azblob_complete_copy_block_list_request(path, block_ids, args)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub fn azblob_head_blob_request(&self, path: &str, args: &OpStat) -> Result<Request<Buffer>> {
        let mut url = self.build_path_url(path);
        if let Some(version) = args.version() {
            url = QueryPairsWriter::new(&url)
                .push("versionid", &percent_encode_path(version))
                .finish();
        }

        let mut req = Request::head(url);

        // Set SSE headers.
        req = self.insert_sse_headers(req);

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetBlobProperties"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn azblob_get_blob_properties(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpStat,
    ) -> Result<Response<Buffer>> {
        let req = self.azblob_head_blob_request(path, args)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    fn azblob_delete_blob_request(&self, path: &str, args: &OpDelete) -> Result<Request<Buffer>> {
        let mut req = Request::delete(self.build_path_url(path));
        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }
        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        req.header(CONTENT_LENGTH, 0)
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteBlob"))
            .body(Buffer::new())
            .map_err(new_request_build_error)
    }

    pub async fn azblob_delete_blob(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpDelete,
    ) -> Result<Response<Buffer>> {
        let req = self.azblob_delete_blob_request(path, args)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azblob_copy_blob(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Response<Buffer>> {
        let mut source = self.build_path_url(from);
        if let Some(version) = args.source_version() {
            source = QueryPairsWriter::new(&source)
                .push("versionid", &percent_encode_path(version))
                .finish();
        }
        let target = self.build_path_url(to);

        let mut req = Request::put(&target)
            .header(constants::X_MS_COPY_SOURCE, source)
            .header(CONTENT_LENGTH, 0);

        // Add if_not_exists condition using If-None-Match header
        if args.if_not_exists() {
            req = req.header(IF_NONE_MATCH, "*");
        }
        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }
        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        let req = req
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyBlob"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azblob_list_blobs(
        &self,
        ctx: &OperationContext,
        path: &str,
        next_marker: &str,
        delimiter: &str,
        limit: Option<usize>,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);
        let mut url = QueryPairsWriter::new(&format!("{}/{}", self.endpoint, self.container))
            .push("restype", "container")
            .push("comp", "list");

        if !p.is_empty() {
            url = url.push("prefix", &percent_encode_path(&p));
        }
        if let Some(limit) = limit {
            url = url.push("maxresults", &limit.to_string());
        }
        if !delimiter.is_empty() {
            url = url.push("delimiter", delimiter);
        }
        if !next_marker.is_empty() {
            url = url.push("marker", &percent_encode_path(next_marker));
        }

        let req = Request::get(url.finish())
            .extension(Operation::List)
            .extension(ServiceOperation("ListBlobs"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azblob_batch_delete(
        &self,
        ctx: &OperationContext,
        batch: &[(String, OpDelete)],
    ) -> Result<Response<Buffer>> {
        let url = format!(
            "{}/{}?restype=container&comp=batch",
            self.endpoint, self.container
        );

        let mut multipart = Multipart::new();

        for (idx, (path, args)) in batch.iter().enumerate() {
            let req = self.azblob_delete_blob_request(path, args)?;
            let req = self.batch_sign(ctx, req).await?;

            multipart = multipart.part(
                MixedPart::from_request(req).part_header("content-id".parse().unwrap(), idx.into()),
            );
        }

        let req = Request::post(url)
            .extension(Operation::Delete)
            .extension(ServiceOperation("BatchDeleteBlobs"));
        let req = multipart.apply(req)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }
}

/// Request of PutBlockListRequest
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(default, rename = "BlockList", rename_all = "PascalCase")]
pub struct PutBlockListRequest {
    pub latest: Vec<String>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct ListBlobsOutput {
    pub blobs: Blobs,
    #[serde(rename = "NextMarker")]
    pub next_marker: Option<String>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Blobs {
    pub blob: Vec<Blob>,
    pub blob_prefix: Vec<BlobPrefix>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct BlobPrefix {
    pub name: String,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Blob {
    pub properties: Properties,
    pub name: String,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Properties {
    #[serde(rename = "Content-Length")]
    pub content_length: u64,
    #[serde(rename = "Last-Modified")]
    pub last_modified: String,
    #[serde(rename = "Content-MD5")]
    pub content_md5: String,
    #[serde(rename = "Content-Type")]
    pub content_type: String,
    pub etag: String,
}

#[cfg(test)]
mod tests {
    use bytes::Buf;
    use bytes::Bytes;
    use quick_xml::de;

    use super::*;
    use std::collections::HashMap;

    use reqsign_azure_storage::RequestSigner;
    use reqsign_azure_storage::StaticCredentialProvider;

    fn test_core() -> AzblobCore {
        AzblobCore {
            info: ServiceInfo::new("azblob", "/", "c"),
            capability: Capability::default(),
            container: "c".to_string(),
            root: "/".to_string(),
            endpoint: "https://acc.blob.core.windows.net".to_string(),
            encryption_key: None,
            encryption_key_sha256: None,
            encryption_algorithm: None,
            skip_signature: true,
            signer: Signer::new(
                Context::new(),
                StaticCredentialProvider::new_shared_key("acc", "a2V5"),
                RequestSigner::new(),
            ),
        }
    }

    #[test]
    fn test_parse_xml() {
        let bs = bytes::Bytes::from(
            r#"
            <?xml version="1.0" encoding="utf-8"?>
            <EnumerationResults ServiceEndpoint="https://test.blob.core.windows.net/" ContainerName="myazurebucket">
                <Prefix>dir1/</Prefix>
                <Delimiter>/</Delimiter>
                <Blobs>
                    <Blob>
                        <Name>dir1/2f018bb5-466f-4af1-84fa-2b167374ee06</Name>
                        <Properties>
                            <Creation-Time>Sun, 20 Mar 2022 11:29:03 GMT</Creation-Time>
                            <Last-Modified>Sun, 20 Mar 2022 11:29:03 GMT</Last-Modified>
                            <Etag>0x8DA0A64D66790C3</Etag>
                            <Content-Length>3485277</Content-Length>
                            <Content-Type>application/octet-stream</Content-Type>
                            <Content-Encoding />
                            <Content-Language />
                            <Content-CRC64 />
                            <Content-MD5>llJ/+jOlx5GdA1sL7SdKuw==</Content-MD5>
                            <Cache-Control />
                            <Content-Disposition />
                            <BlobType>BlockBlob</BlobType>
                            <AccessTier>Hot</AccessTier>
                            <AccessTierInferred>true</AccessTierInferred>
                            <LeaseStatus>unlocked</LeaseStatus>
                            <LeaseState>available</LeaseState>
                            <ServerEncrypted>true</ServerEncrypted>
                        </Properties>
                        <OrMetadata />
                    </Blob>
                    <Blob>
                        <Name>dir1/5b9432b2-79c0-48d8-90c2-7d3e153826ed</Name>
                        <Properties>
                            <Creation-Time>Tue, 29 Mar 2022 01:54:07 GMT</Creation-Time>
                            <Last-Modified>Tue, 29 Mar 2022 01:54:07 GMT</Last-Modified>
                            <Etag>0x8DA112702D88FE4</Etag>
                            <Content-Length>2471869</Content-Length>
                            <Content-Type>application/octet-stream</Content-Type>
                            <Content-Encoding />
                            <Content-Language />
                            <Content-CRC64 />
                            <Content-MD5>xmgUltSnopLSJOukgCHFtg==</Content-MD5>
                            <Cache-Control />
                            <Content-Disposition />
                            <BlobType>BlockBlob</BlobType>
                            <AccessTier>Hot</AccessTier>
                            <AccessTierInferred>true</AccessTierInferred>
                            <LeaseStatus>unlocked</LeaseStatus>
                            <LeaseState>available</LeaseState>
                            <ServerEncrypted>true</ServerEncrypted>
                        </Properties>
                        <OrMetadata />
                    </Blob>
                    <Blob>
                        <Name>dir1/b2d96f8b-d467-40d1-bb11-4632dddbf5b5</Name>
                        <Properties>
                            <Creation-Time>Sun, 20 Mar 2022 11:31:57 GMT</Creation-Time>
                            <Last-Modified>Sun, 20 Mar 2022 11:31:57 GMT</Last-Modified>
                            <Etag>0x8DA0A653DC82981</Etag>
                            <Content-Length>1259677</Content-Length>
                            <Content-Type>application/octet-stream</Content-Type>
                            <Content-Encoding />
                            <Content-Language />
                            <Content-CRC64 />
                            <Content-MD5>AxTiFXHwrXKaZC5b7ZRybw==</Content-MD5>
                            <Cache-Control />
                            <Content-Disposition />
                            <BlobType>BlockBlob</BlobType>
                            <AccessTier>Hot</AccessTier>
                            <AccessTierInferred>true</AccessTierInferred>
                            <LeaseStatus>unlocked</LeaseStatus>
                            <LeaseState>available</LeaseState>
                            <ServerEncrypted>true</ServerEncrypted>
                        </Properties>
                        <OrMetadata />
                    </Blob>
                    <BlobPrefix>
                        <Name>dir1/dir2/</Name>
                    </BlobPrefix>
                    <BlobPrefix>
                        <Name>dir1/dir21/</Name>
                    </BlobPrefix>
                </Blobs>
                <NextMarker />
            </EnumerationResults>"#,
        );
        let out: ListBlobsOutput = de::from_reader(bs.reader()).expect("must success");
        println!("{out:?}");

        assert_eq!(
            out.blobs
                .blob
                .iter()
                .map(|v| v.name.clone())
                .collect::<Vec<String>>(),
            [
                "dir1/2f018bb5-466f-4af1-84fa-2b167374ee06",
                "dir1/5b9432b2-79c0-48d8-90c2-7d3e153826ed",
                "dir1/b2d96f8b-d467-40d1-bb11-4632dddbf5b5"
            ]
        );
        assert_eq!(
            out.blobs
                .blob
                .iter()
                .map(|v| v.properties.content_length)
                .collect::<Vec<u64>>(),
            [3485277, 2471869, 1259677]
        );
        assert_eq!(
            out.blobs
                .blob
                .iter()
                .map(|v| v.properties.content_md5.clone())
                .collect::<Vec<String>>(),
            [
                "llJ/+jOlx5GdA1sL7SdKuw==".to_string(),
                "xmgUltSnopLSJOukgCHFtg==".to_string(),
                "AxTiFXHwrXKaZC5b7ZRybw==".to_string()
            ]
        );
        assert_eq!(
            out.blobs
                .blob
                .iter()
                .map(|v| v.properties.last_modified.clone())
                .collect::<Vec<String>>(),
            [
                "Sun, 20 Mar 2022 11:29:03 GMT".to_string(),
                "Tue, 29 Mar 2022 01:54:07 GMT".to_string(),
                "Sun, 20 Mar 2022 11:31:57 GMT".to_string()
            ]
        );
        assert_eq!(
            out.blobs
                .blob
                .iter()
                .map(|v| v.properties.etag.clone())
                .collect::<Vec<String>>(),
            [
                "0x8DA0A64D66790C3".to_string(),
                "0x8DA112702D88FE4".to_string(),
                "0x8DA0A653DC82981".to_string()
            ]
        );
        assert_eq!(
            out.blobs
                .blob_prefix
                .iter()
                .map(|v| v.name.clone())
                .collect::<Vec<String>>(),
            ["dir1/dir2/", "dir1/dir21/"]
        );
    }

    /// This case is copied from real environment for testing
    /// quick-xml overlapped-lists features. By default, quick-xml
    /// can't deserialize content with overlapped-lists.
    ///
    /// For example, this case list blobs in this way:
    ///
    /// ```xml
    /// <Blobs>
    ///     <Blob>xxx</Blob>
    ///     <BlobPrefix>yyy</BlobPrefix>
    ///     <Blob>zzz</Blob>
    /// </Blobs>
    /// ```
    ///
    /// If `overlapped-lists` feature not enabled, we will get error `duplicate field Blob`.
    #[test]
    fn test_parse_overlapped_lists() {
        let bs = "<?xml version=\"1.0\" encoding=\"utf-8\"?><EnumerationResults ServiceEndpoint=\"https://test.blob.core.windows.net/\" ContainerName=\"test\"><Prefix>9f7075e1-84d0-45ca-8196-ab9b71a8ef97/x/</Prefix><Delimiter>/</Delimiter><Blobs><Blob><Name>9f7075e1-84d0-45ca-8196-ab9b71a8ef97/x/</Name><Properties><Creation-Time>Thu, 01 Sep 2022 07:26:49 GMT</Creation-Time><Last-Modified>Thu, 01 Sep 2022 07:26:49 GMT</Last-Modified><Etag>0x8DA8BEB55D0EA35</Etag><Content-Length>0</Content-Length><Content-Type>application/octet-stream</Content-Type><Content-Encoding /><Content-Language /><Content-CRC64 /><Content-MD5>1B2M2Y8AsgTpgAmY7PhCfg==</Content-MD5><Cache-Control /><Content-Disposition /><BlobType>BlockBlob</BlobType><AccessTier>Hot</AccessTier><AccessTierInferred>true</AccessTierInferred><LeaseStatus>unlocked</LeaseStatus><LeaseState>available</LeaseState><ServerEncrypted>true</ServerEncrypted></Properties><OrMetadata /></Blob><BlobPrefix><Name>9f7075e1-84d0-45ca-8196-ab9b71a8ef97/x/x/</Name></BlobPrefix><Blob><Name>9f7075e1-84d0-45ca-8196-ab9b71a8ef97/x/y</Name><Properties><Creation-Time>Thu, 01 Sep 2022 07:26:50 GMT</Creation-Time><Last-Modified>Thu, 01 Sep 2022 07:26:50 GMT</Last-Modified><Etag>0x8DA8BEB55D99C08</Etag><Content-Length>0</Content-Length><Content-Type>application/octet-stream</Content-Type><Content-Encoding /><Content-Language /><Content-CRC64 /><Content-MD5>1B2M2Y8AsgTpgAmY7PhCfg==</Content-MD5><Cache-Control /><Content-Disposition /><BlobType>BlockBlob</BlobType><AccessTier>Hot</AccessTier><AccessTierInferred>true</AccessTierInferred><LeaseStatus>unlocked</LeaseStatus><LeaseState>available</LeaseState><ServerEncrypted>true</ServerEncrypted></Properties><OrMetadata /></Blob></Blobs><NextMarker /></EnumerationResults>";

        de::from_reader(Bytes::from(bs).reader()).expect("must success")
    }

    /// This example is from https://learn.microsoft.com/en-us/rest/api/storageservices/put-block-list?tabs=microsoft-entra-id
    /// Put Block List is the commit point of a blocked write, and the only request
    /// in that sequence where Azure applies blob properties and metadata. azblob
    /// declares `write_with_content_type` and `write_with_user_metadata`, so both
    /// have to be carried here as well as on the one-shot Put Blob path.
    #[test]
    fn test_put_block_list_carries_content_type_and_user_metadata() {
        let core = test_core();

        let mut meta = HashMap::new();
        meta.insert("k".to_string(), "v".to_string());
        let (args, _) = OpWrite::from_options(
            &Capability::default(),
            options::WriteOptions {
                content_type: Some("application/json".to_owned()),
                user_metadata: Some(meta),
                ..Default::default()
            },
        )
        .unwrap();

        let req = core
            .azblob_complete_put_block_list_request("a.txt", vec![Uuid::nil()], &args)
            .expect("must build");

        assert_eq!(
            req.headers()
                .get(constants::X_MS_BLOB_CONTENT_TYPE)
                .map(|v| v.to_str().unwrap()),
            Some("application/json")
        );
        assert_eq!(
            req.headers()
                .get("x-ms-meta-k")
                .map(|v| v.to_str().unwrap()),
            Some("v")
        );
    }

    #[test]
    fn init_appendable_blob_carries_if_not_exists() {
        let core = test_core();
        let (args, _) = OpWrite::from_options(
            &Capability {
                write_with_if_not_exists: true,
                ..Default::default()
            },
            options::WriteOptions {
                if_not_exists: true,
                ..Default::default()
            },
        )
        .expect("options must lower");

        let req = core
            .azblob_init_appendable_blob_request("a.txt", &args)
            .expect("request must build");

        assert_eq!(
            req.headers().get(IF_NONE_MATCH),
            Some(&HeaderValue::from_static("*"))
        );
    }

    #[test]
    fn test_serialize_put_block_list_request() {
        let req = PutBlockListRequest {
            latest: vec!["1".to_string(), "2".to_string(), "3".to_string()],
        };

        let actual = quick_xml::se::to_string(&req).expect("must succeed");

        pretty_assertions::assert_eq!(
            actual,
            r#"
            <BlockList>
               <Latest>1</Latest>
               <Latest>2</Latest>
               <Latest>3</Latest>
            </BlockList>"#
                // Cleanup space and new line
                .replace([' ', '\n'], "")
                // Escape `"` by hand to address <https://github.com/tafia/quick-xml/issues/362>
                .replace('"', "&quot;")
        );

        let bs = "<?xml version=\"1.0\" encoding=\"utf-8\"?>
            <BlockList>
               <Latest>1</Latest>
               <Latest>2</Latest>
               <Latest>3</Latest>
            </BlockList>";

        let out: PutBlockListRequest =
            de::from_reader(Bytes::from(bs).reader()).expect("must success");
        assert_eq!(
            out.latest,
            vec!["1".to_string(), "2".to_string(), "3".to_string()]
        );
    }
}

/// AzblobError is the error returned by azure blob service.
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct AzblobError {
    code: String,
    message: String,
    query_parameter_name: String,
    query_parameter_value: String,
    reason: String,
}

impl Debug for AzblobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut de = f.debug_struct("AzblobError");
        de.field("code", &self.code);
        // replace `\n` to ` ` for better reading.
        de.field("message", &self.message.replace('\n', " "));

        if !self.query_parameter_name.is_empty() {
            de.field("query_parameter_name", &self.query_parameter_name);
        }
        if !self.query_parameter_value.is_empty() {
            de.field("query_parameter_value", &self.query_parameter_value);
        }
        if !self.reason.is_empty() {
            de.field("reason", &self.reason);
        }

        de.finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ErrorContext {
    service_operation: ServiceOperation,
    caller_condition: bool,
    if_not_exists: bool,
    is_append_blob_initialization: bool,
}

impl ErrorContext {
    pub const fn new(service_operation: ServiceOperation) -> Self {
        Self {
            service_operation,
            caller_condition: false,
            if_not_exists: false,
            is_append_blob_initialization: false,
        }
    }

    pub const fn with_caller_condition(mut self, caller_condition: bool) -> Self {
        self.caller_condition = caller_condition;
        self
    }

    pub const fn with_if_not_exists(mut self, if_not_exists: bool) -> Self {
        self.caller_condition = self.caller_condition || if_not_exists;
        self.if_not_exists = if_not_exists;
        self
    }

    pub const fn with_append_blob_initialization(
        mut self,
        is_append_blob_initialization: bool,
    ) -> Self {
        self.is_append_blob_initialization = is_append_blob_initialization;
        self
    }
}

/// Parse error response into Error.
pub fn parse_error(ctx: ErrorContext, resp: Response<Buffer>) -> Error {
    let (parts, body) = resp.into_parts();
    let bs = body.to_bytes();

    let bs_content = bs.chunk();
    let mut azblob_error = de::from_reader::<_, AzblobError>(bs_content.reader()).ok();

    if azblob_error.as_ref().is_none_or(|err| err.code.is_empty())
        && let Some(code) = parts
            .headers
            .get("x-ms-error-code")
            .and_then(|v| v.to_str().ok())
    {
        azblob_error.get_or_insert_with(AzblobError::default).code = code.to_string();
    }

    let (mut kind, mut retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        StatusCode::NOT_MODIFIED | StatusCode::PRECONDITION_FAILED if ctx.caller_condition => {
            (ErrorKind::ConditionNotMatch, false)
        }
        StatusCode::TOO_MANY_REQUESTS => (ErrorKind::RateLimited, true),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    if let Some(azblob_error) = &azblob_error
        && !azblob_error.code.is_empty()
    {
        if let Some(classification) = parse_azblob_error_code(ctx, &azblob_error.code, parts.status)
        {
            (kind, retryable) = classification;
        } else if matches!(
            parts.status,
            StatusCode::CONFLICT | StatusCode::PRECONDITION_FAILED
        ) {
            (kind, retryable) = (ErrorKind::Unexpected, false);
        }
    }

    let message = azblob_error
        .map(|err| format!("{err:?}"))
        .unwrap_or_else(|| String::from_utf8_lossy(&bs).into_owned());

    let mut err =
        Error::new(kind, &message).with_context("service_operation", ctx.service_operation.0);

    err = with_azure_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}

/// Classify documented Blob Storage state errors by native code and status.
///
/// Reference: <https://learn.microsoft.com/rest/api/storageservices/blob-service-error-codes>
fn parse_azblob_error_code(
    ctx: ErrorContext,
    code: &str,
    status: StatusCode,
) -> Option<(ErrorKind, bool)> {
    match code {
        "ConditionNotMet" | "SourceConditionNotMet" | "TargetConditionNotMet"
            if ctx.caller_condition
                && matches!(
                    status,
                    StatusCode::NOT_MODIFIED | StatusCode::PRECONDITION_FAILED
                ) =>
        {
            Some((ErrorKind::ConditionNotMatch, false))
        }
        "BlobAlreadyExists" | "ContainerAlreadyExists" | "ResourceAlreadyExists" => {
            let classification = if ctx.if_not_exists {
                (ErrorKind::ConditionNotMatch, false)
            } else if ctx.is_append_blob_initialization {
                (ErrorKind::Conflict, true)
            } else {
                (ErrorKind::AlreadyExists, false)
            };
            Some(classification)
        }
        "DirectorySasNotSupportedVersion"
        | "FeatureVersionMismatch"
        | "PreviousSnapshotOperationNotSupported" => Some((ErrorKind::Unsupported, false)),
        "SnapshotOperationRateExceeded" => Some((ErrorKind::RateLimited, true)),
        "AppendPositionConditionNotMet"
        | "BlobArchived"
        | "BlobBeingRehydrated"
        | "BlobImmutableDueToLegalHold"
        | "BlobImmutableDueToPolicy"
        | "BlobNotArchived"
        | "BlobOperationNotSupported"
        | "BlobOverwritten"
        | "BlobTierInadequateForContentLength"
        | "BlockCountExceedsLimit"
        | "CannotChangeToLowerTier"
        | "ContainerBeingDeleted"
        | "ContainerDisabled"
        | "ContainerHasLegalHold"
        | "ContainerImmutabilityPolicyLocked"
        | "ContentLengthLargerThanTierLimit"
        | "CopyIdMismatch"
        | "IncrementalCopyBlobMismatch"
        | "IncrementalCopyOfEarlierSnapshotNotAllowed"
        | "IncrementalCopyOfEarlierVersionSnapshotNotAllowed"
        | "IncrementalCopySourceMustBeSnapshot"
        | "InvalidBlobType"
        | "InvalidSourceBlobType"
        | "InvalidSourceBlobUrl"
        | "LeaseAlreadyBroken"
        | "LeaseAlreadyPresent"
        | "LeaseIdMismatchWithBlobOperation"
        | "LeaseIdMismatchWithContainerOperation"
        | "LeaseIdMismatchWithLeaseOperation"
        | "LeaseIdMissing"
        | "LeaseIsBreakingAndCannotBeAcquired"
        | "LeaseIsBreakingAndCannotBeChanged"
        | "LeaseIsBrokenAndCannotBeRenewed"
        | "LeaseLost"
        | "LeaseNotPresentWithBlobOperation"
        | "LeaseNotPresentWithContainerOperation"
        | "LeaseNotPresentWithLeaseOperation"
        | "InfiniteLeaseDurationRequired"
        | "MaxBlobSizeConditionNotMet"
        | "NoPendingCopyOperation"
        | "OperationNotAllowedOnIncrementalCopyBlob"
        | "PendingCopyOperation"
        | "PreviousSnapshotNotFound"
        | "SequenceNumberConditionNotMet"
        | "SequenceNumberIncrementTooLarge"
        | "SnapshotCountExceeded"
        | "SnapshotsPresent"
        | "SystemInUse"
        | "ResourceTypeMismatch"
            if matches!(
                status,
                StatusCode::CONFLICT | StatusCode::PRECONDITION_FAILED
            ) =>
        {
            Some((ErrorKind::Conflict, false))
        }
        _ => None,
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn append_blob_initialization_preserves_condition_and_retryability() {
        let conditional = ErrorContext::new(ServiceOperation("PutBlob"))
            .with_if_not_exists(true)
            .with_append_blob_initialization(true);
        assert_eq!(
            parse_azblob_error_code(conditional, "BlobAlreadyExists", StatusCode::CONFLICT),
            Some((ErrorKind::ConditionNotMatch, false))
        );

        let raced =
            ErrorContext::new(ServiceOperation("PutBlob")).with_append_blob_initialization(true);
        assert_eq!(
            parse_azblob_error_code(raced, "BlobAlreadyExists", StatusCode::CONFLICT),
            Some((ErrorKind::Conflict, true))
        );
    }
}
