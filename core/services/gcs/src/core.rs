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
use std::fmt::Debug;
use std::fmt::Write;

use bytes::Buf;
use bytes::Bytes;
use constants::*;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header::CACHE_CONTROL;
use http::header::CONTENT_DISPOSITION;
use http::header::CONTENT_ENCODING;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_RANGE;
use http::header::CONTENT_TYPE;
use http::header::HOST;
use http::header::IF_MATCH;
use http::header::IF_MODIFIED_SINCE;
use http::header::IF_NONE_MATCH;
use http::header::IF_UNMODIFIED_SINCE;
use reqsign_core::{Context, Signer};
use reqsign_google::Credential;
use serde::Deserialize;
use serde::Serialize;
use serde_json::de;

use opendal_core::raw::*;
use opendal_core::*;

pub mod constants {
    pub const X_GOOG_GENERATION: &str = "x-goog-generation";
    pub const GCS_REWRITE_MIN_CHUNK_SIZE: usize = 1024 * 1024;
    #[cfg(target_pointer_width = "64")]
    pub const GCS_REWRITE_MAX_CHUNK_SIZE: usize =
        i64::MAX as usize / GCS_REWRITE_MIN_CHUNK_SIZE * GCS_REWRITE_MIN_CHUNK_SIZE;
    #[cfg(not(target_pointer_width = "64"))]
    pub const GCS_REWRITE_MAX_CHUNK_SIZE: usize =
        usize::MAX / GCS_REWRITE_MIN_CHUNK_SIZE * GCS_REWRITE_MIN_CHUNK_SIZE;

    pub const X_GOOG_ACL: &str = "x-goog-acl";
    pub const X_GOOG_STORAGE_CLASS: &str = "x-goog-storage-class";
    pub const X_GOOG_META_PREFIX: &str = "x-goog-meta-";

    pub const RESPONSE_CONTENT_DISPOSITION: &str = "response-content-disposition";
    pub const RESPONSE_CONTENT_TYPE: &str = "response-content-type";
    pub const RESPONSE_CACHE_CONTROL: &str = "response-cache-control";
}

pub struct GcsCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub endpoint: String,
    pub bucket: String,
    pub root: String,

    pub signer: Signer<Credential>,
    pub sign_ctx: Context,

    pub predefined_acl: Option<String>,
    pub default_storage_class: Option<String>,

    pub skip_signature: bool,
}

impl Debug for GcsCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsCore")
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl GcsCore {
    fn signer(&self, ctx: &OperationContext) -> Signer<Credential> {
        self.signer.clone().with_context(
            self.sign_ctx
                .clone()
                .with_http_send(ctx.http_transport().clone()),
        )
    }

    pub async fn sign<T>(&self, ctx: &OperationContext, req: Request<T>) -> Result<Request<T>> {
        if self.skip_signature {
            return Ok(req);
        }

        let (mut parts, body) = req.into_parts();

        self.signer(ctx)
            .sign(&mut parts, None)
            .await
            .map_err(|err| new_request_sign_error(err.into()))?;

        // Always remove host header, let users' client to set it based on
        // HTTP version.
        //
        // As discussed in <https://github.com/seanmonstar/reqwest/issues/1809>,
        // google server could send RST_STREAM of PROTOCOL_ERROR if our
        // request contains host header.
        parts.headers.remove(HOST);

        Ok(Request::from_parts(parts, body))
    }

    pub async fn sign_query<T>(
        &self,
        ctx: &OperationContext,
        req: Request<T>,
        duration: Duration,
    ) -> Result<Request<T>> {
        if self.skip_signature {
            return Ok(req);
        }

        let (mut parts, body) = req.into_parts();

        self.signer(ctx)
            .sign(&mut parts, Some(duration))
            .await
            .map_err(|err| new_request_sign_error(err.into()))?;

        // Always remove host header, let users' client to set it based on
        // HTTP version.
        //
        // As discussed in <https://github.com/seanmonstar/reqwest/issues/1809>,
        // google server could send RST_STREAM of PROTOCOL_ERROR if our
        // request contains host header.
        parts.headers.remove(HOST);

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
}

impl GcsCore {
    pub fn gcs_get_object_request(
        &self,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/storage/v1/b/{}/o/{}",
            self.endpoint,
            self.bucket,
            gcs_percent_encode_path(&p)
        );

        let mut url = QueryPairsWriter::new(&url).push("alt", "media");
        if let Some(version) = args.version() {
            url = url.push("generation", &gcs_percent_encode_path(version));
        }
        if let Some(version) = args.if_version_match() {
            url = url.push("ifGenerationMatch", &gcs_percent_encode_path(version));
        }
        if let Some(version) = args.if_version_not_match() {
            url = url.push("ifGenerationNotMatch", &gcs_percent_encode_path(version));
        }
        let url = url.finish();

        let mut req = Request::get(&url);

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }
        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }
        if !range.is_full() {
            req = req.header(http::header::RANGE, range.to_header());
        }

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("GetObject"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    // It's for presign operation. Gcs only supports query sign over XML API.
    pub fn gcs_get_object_xml_request(
        &self,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!("{}/{}/{}", self.endpoint, self.bucket, p);

        let mut query_args = Vec::new();
        if let Some(version) = args.version() {
            query_args.push(format!("generation={}", percent_encode_path(version)))
        }
        if let Some(override_content_disposition) = args.override_content_disposition() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CONTENT_DISPOSITION,
                percent_encode_path(override_content_disposition)
            ))
        }
        if let Some(override_content_type) = args.override_content_type() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CONTENT_TYPE,
                percent_encode_path(override_content_type)
            ))
        }
        if let Some(override_cache_control) = args.override_cache_control() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CACHE_CONTROL,
                percent_encode_path(override_cache_control)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        let mut req = Request::get(&url);

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }
        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }
        if let Some(version) = args.if_version_match() {
            req = req.header("x-goog-if-generation-match", version);
        }

        if let Some(if_modified_since) = args.if_modified_since() {
            req = req.header(IF_MODIFIED_SINCE, if_modified_since.format_http_date());
        }

        if let Some(if_unmodified_since) = args.if_unmodified_since() {
            req = req.header(IF_UNMODIFIED_SINCE, if_unmodified_since.format_http_date());
        }
        if !range.is_full() {
            req = req.header(http::header::RANGE, range.to_header());
        }

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("GetObject"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn gcs_get_object(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Response<HttpBody>> {
        let req = self.gcs_get_object_request(path, range, args)?;

        let req = self.sign(ctx, req).await?;
        ctx.http_transport().fetch(req).await
    }

    pub fn gcs_insert_object_request(
        &self,
        path: &str,
        size: Option<u64>,
        op: &OpWrite,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let request_metadata = InsertRequestMetadata {
            storage_class: self.default_storage_class.as_deref(),
            cache_control: op.cache_control(),
            content_type: op.content_type(),
            content_encoding: op.content_encoding(),
            metadata: op.user_metadata(),
        };

        let mut url = format!(
            "{}/upload/storage/v1/b/{}/o?uploadType={}&name={}",
            self.endpoint,
            self.bucket,
            if request_metadata.is_empty() {
                "media"
            } else {
                "multipart"
            },
            gcs_percent_encode_path(&p)
        );

        if let Some(acl) = &self.predefined_acl {
            write!(&mut url, "&predefinedAcl={acl}").unwrap();
        }

        if let Some(version) = op.if_version_match() {
            write!(
                &mut url,
                "&ifGenerationMatch={}",
                gcs_percent_encode_path(version)
            )
            .unwrap();
        } else if op.if_not_exists() {
            write!(&mut url, "&ifGenerationMatch=0").unwrap();
        }
        if let Some(version) = op.if_version_not_match() {
            write!(
                &mut url,
                "&ifGenerationNotMatch={}",
                gcs_percent_encode_path(version)
            )
            .unwrap();
        }

        let mut req = Request::post(&url);

        req = req.header(CONTENT_LENGTH, size.unwrap_or_default());

        if request_metadata.is_empty() {
            let req = req
                .extension(Operation::Write)
                .extension(ServiceOperation("InsertObject"));
            // If the metadata is empty, we do not set any `Content-Type` header,
            // since if we had it in the `op.content_type()`, it would be already set in the
            // `multipart` metadata body and this branch won't be executed.
            let req = req.body(body).map_err(new_request_build_error)?;
            Ok(req)
        } else {
            let mut multipart = Multipart::new();
            let metadata_part = RelatedPart::new()
                .header(
                    CONTENT_TYPE,
                    "application/json; charset=UTF-8".parse().unwrap(),
                )
                .content(
                    serde_json::to_vec(&request_metadata)
                        .expect("metadata serialization should succeed"),
                );
            multipart = multipart.part(metadata_part);

            // Content-Type must be set, even if it is set in the metadata part
            let content_type = op
                .content_type()
                .unwrap_or("application/octet-stream")
                .parse()
                .expect("Failed to parse content-type");
            let media_part = RelatedPart::new()
                .header(CONTENT_TYPE, content_type)
                .content(body);
            multipart = multipart.part(media_part);

            let req = multipart.apply(
                Request::post(url)
                    .extension(Operation::Write)
                    .extension(ServiceOperation("InsertObject")),
            )?;

            Ok(req)
        }
    }

    pub fn gcs_initiate_resumable_upload_request(
        &self,
        path: &str,
        op: &OpWrite,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);
        let base = format!("{}/upload/storage/v1/b/{}/o", self.endpoint, self.bucket);
        let mut url = QueryPairsWriter::new(&base)
            .push("uploadType", "resumable")
            .push("name", &gcs_percent_encode_path(&p));

        if let Some(acl) = &self.predefined_acl {
            url = url.push("predefinedAcl", acl);
        }
        if let Some(version) = op.if_version_match() {
            url = url.push("ifGenerationMatch", &gcs_percent_encode_path(version));
        } else if op.if_not_exists() {
            url = url.push("ifGenerationMatch", "0");
        }
        if let Some(version) = op.if_version_not_match() {
            url = url.push("ifGenerationNotMatch", &gcs_percent_encode_path(version));
        }

        let metadata = InsertRequestMetadata {
            storage_class: self.default_storage_class.as_deref(),
            cache_control: op.cache_control(),
            content_type: op.content_type(),
            content_encoding: op.content_encoding(),
            metadata: op.user_metadata(),
        };
        let body = serde_json::to_vec(&metadata).map_err(new_json_serialize_error)?;

        let mut req = Request::post(url.finish())
            .header(CONTENT_TYPE, "application/json; charset=UTF-8")
            .header(CONTENT_LENGTH, body.len());
        if let Some(content_type) = op.content_type() {
            req = req.header("x-upload-content-type", content_type);
        }

        req.extension(Operation::Write)
            .extension(ServiceOperation("InitiateResumableUpload"))
            .body(Buffer::from(Bytes::from(body)))
            .map_err(new_request_build_error)
    }

    pub async fn gcs_initiate_resumable_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        op: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let req = self.gcs_initiate_resumable_upload_request(path, op)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn gcs_upload_resumable_chunk(
        &self,
        ctx: &OperationContext,
        session_uri: &str,
        offset: u64,
        body: Buffer,
        total: Option<u64>,
    ) -> Result<Response<Buffer>> {
        let end = offset + body.len() as u64;
        let mut content_range = BytesContentRange::default();
        if !body.is_empty() {
            content_range = content_range.with_range(offset, end - 1);
        }
        if let Some(total) = total {
            content_range = content_range.with_size(total);
        }
        let req = Request::put(session_uri)
            .header(CONTENT_LENGTH, body.len())
            .header(CONTENT_RANGE, content_range.to_header())
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadResumableChunk"))
            .body(body)
            .map_err(new_request_build_error)?;
        self.send(ctx, req).await
    }

    pub async fn gcs_query_resumable_upload(
        &self,
        ctx: &OperationContext,
        session_uri: &str,
        total: u64,
    ) -> Result<Response<Buffer>> {
        let content_range = BytesContentRange::default().with_size(total);
        let req = Request::put(session_uri)
            .header(CONTENT_LENGTH, 0)
            .header(CONTENT_RANGE, content_range.to_header())
            .extension(Operation::Write)
            .extension(ServiceOperation("QueryResumableUpload"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        self.send(ctx, req).await
    }

    pub async fn gcs_cancel_resumable_upload(
        &self,
        ctx: &OperationContext,
        session_uri: &str,
    ) -> Result<Response<Buffer>> {
        let req = Request::delete(session_uri)
            .header(CONTENT_LENGTH, 0)
            .extension(Operation::Write)
            .extension(ServiceOperation("CancelResumableUpload"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        self.send(ctx, req).await
    }

    // It's for presign operation. Gcs only supports query sign over XML API.
    pub fn gcs_insert_object_xml_request(
        &self,
        path: &str,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}/{}", self.endpoint, self.bucket, p);

        let mut req = Request::put(&url);

        if let Some(user_metadata) = args.user_metadata() {
            for (key, value) in user_metadata {
                req = req.header(format!("{X_GOOG_META_PREFIX}{key}"), value)
            }
        }

        if let Some(content_type) = args.content_type() {
            req = req.header(CONTENT_TYPE, content_type);
        }

        if let Some(content_encoding) = args.content_encoding() {
            req = req.header(CONTENT_ENCODING, content_encoding);
        }

        if let Some(acl) = &self.predefined_acl {
            if let Some(predefined_acl_in_xml_spec) = predefined_acl_to_xml_header(acl) {
                req = req.header(X_GOOG_ACL, predefined_acl_in_xml_spec);
            } else {
                log::warn!("Unrecognized predefined_acl. Ignoring");
            }
        }

        if let Some(storage_class) = &self.default_storage_class {
            req = req.header(X_GOOG_STORAGE_CLASS, storage_class);
        }

        if let Some(version) = args.if_version_match() {
            req = req.header("x-goog-if-generation-match", version);
        } else if args.if_not_exists() {
            req = req.header("x-goog-if-generation-match", "0");
        }
        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }
        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("InsertObject"));

        let req = req.body(body).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub fn gcs_head_object_request(&self, path: &str, args: &OpStat) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/storage/v1/b/{}/o/{}",
            self.endpoint,
            self.bucket,
            gcs_percent_encode_path(&p)
        );

        let mut url = QueryPairsWriter::new(&url);
        if let Some(version) = args.version() {
            url = url.push("generation", &gcs_percent_encode_path(version));
        }
        if let Some(version) = args.if_version_match() {
            url = url.push("ifGenerationMatch", &gcs_percent_encode_path(version));
        }
        if let Some(version) = args.if_version_not_match() {
            url = url.push("ifGenerationNotMatch", &gcs_percent_encode_path(version));
        }
        let url = url.finish();

        let mut req = Request::get(&url);

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }
        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetObject"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    // It's for presign operation. Gcs only supports query sign over XML API.
    pub fn gcs_head_object_xml_request(
        &self,
        path: &str,
        args: &OpStat,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!("{}/{}/{}", self.endpoint, self.bucket, p);

        let mut query_args = Vec::new();
        if let Some(version) = args.version() {
            query_args.push(format!("generation={}", percent_encode_path(version)))
        }
        if let Some(override_content_disposition) = args.override_content_disposition() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CONTENT_DISPOSITION,
                percent_encode_path(override_content_disposition)
            ))
        }
        if let Some(override_content_type) = args.override_content_type() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CONTENT_TYPE,
                percent_encode_path(override_content_type)
            ))
        }
        if let Some(override_cache_control) = args.override_cache_control() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CACHE_CONTROL,
                percent_encode_path(override_cache_control)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        let mut req = Request::head(&url);

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }
        if let Some(version) = args.if_version_match() {
            req = req.header("x-goog-if-generation-match", version);
        }

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetObject"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn gcs_get_object_metadata(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpStat,
    ) -> Result<Response<Buffer>> {
        let req = self.gcs_head_object_request(path, args)?;

        let req = self.sign(ctx, req).await?;

        self.send(ctx, req).await
    }

    pub async fn gcs_delete_object(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpDelete,
    ) -> Result<Response<Buffer>> {
        let req = self.gcs_delete_object_request(path, args)?;

        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub fn gcs_delete_object_request(
        &self,
        path: &str,
        args: &OpDelete,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/storage/v1/b/{}/o/{}",
            self.endpoint,
            self.bucket,
            gcs_percent_encode_path(&p)
        );

        let mut url = QueryPairsWriter::new(&url);
        if let Some(version) = args.version() {
            url = url.push("generation", &gcs_percent_encode_path(version));
        }
        if let Some(version) = args.if_version_match() {
            url = url.push("ifGenerationMatch", &gcs_percent_encode_path(version));
        }
        if let Some(version) = args.if_version_not_match() {
            url = url.push("ifGenerationNotMatch", &gcs_percent_encode_path(version));
        }
        let url = url.finish();

        Request::delete(&url)
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteObject"))
            .body(Buffer::new())
            .map_err(new_request_build_error)
    }

    pub async fn gcs_delete_objects(
        &self,
        ctx: &OperationContext,
        paths: &[(String, OpDelete)],
    ) -> Result<Response<Buffer>> {
        let req = self.gcs_delete_objects_request(paths)?;

        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub fn gcs_delete_objects_request(
        &self,
        paths: &[(String, OpDelete)],
    ) -> Result<Request<Buffer>> {
        let uri = format!("{}/batch/storage/v1", self.endpoint);

        let mut multipart = Multipart::new();

        for (idx, (path, args)) in paths.iter().enumerate() {
            let req = self.gcs_delete_object_request(path, args)?;

            multipart = multipart.part(
                MixedPart::from_request(req).part_header("content-id".parse().unwrap(), idx.into()),
            );
        }

        let req = Request::post(uri)
            .extension(Operation::Delete)
            .extension(ServiceOperation("BatchDeleteObjects"));
        multipart.apply(req)
    }

    pub fn gcs_compose_object_request(
        &self,
        sources: &[GcsComposeSource],
        to: &str,
        args: &OpCompose,
    ) -> Result<Request<Buffer>> {
        let destination = build_abs_path(&self.root, to);
        let base = format!(
            "{}/storage/v1/b/{}/o/{}/compose",
            self.endpoint,
            self.bucket,
            gcs_percent_encode_path(&destination)
        );
        let mut url = QueryPairsWriter::new(&base);
        if let Some(acl) = &self.predefined_acl {
            url = url.push("destinationPredefinedAcl", acl);
        }
        if let Some(version) = args.if_version_match() {
            url = url.push("ifGenerationMatch", &gcs_percent_encode_path(version));
        } else if args.if_not_exists() {
            url = url.push("ifGenerationMatch", "0");
        }

        let source_objects = sources
            .iter()
            .map(|source| ComposeSourceObject {
                name: build_abs_path(&self.root, &source.path),
                generation: source.version.clone(),
                object_preconditions: source.if_version_match.as_ref().map(|version| {
                    ComposeSourceObjectPreconditions {
                        if_generation_match: version.clone(),
                    }
                }),
            })
            .collect();
        let request = ComposeRequest {
            source_objects,
            destination: ComposeDestination {
                storage_class: self.default_storage_class.as_deref(),
                cache_control: args.cache_control(),
                content_type: Some(args.content_type().unwrap_or("application/octet-stream")),
                content_disposition: args.content_disposition(),
                content_encoding: args.content_encoding(),
                metadata: args.user_metadata(),
            },
            delete_source_objects: false,
        };
        let body = serde_json::to_vec(&request).map_err(new_json_serialize_error)?;

        Request::post(url.finish())
            .header(CONTENT_TYPE, "application/json; charset=UTF-8")
            .header(CONTENT_LENGTH, body.len())
            .extension(Operation::Compose)
            .extension(ServiceOperation("ComposeObject"))
            .body(Buffer::from(Bytes::from(body)))
            .map_err(new_request_build_error)
    }

    pub async fn gcs_compose_object(
        &self,
        ctx: &OperationContext,
        sources: &[GcsComposeSource],
        to: &str,
        args: &OpCompose,
    ) -> Result<Response<Buffer>> {
        let req = self.gcs_compose_object_request(sources, to, args)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub fn gcs_rewrite_object_request(
        &self,
        from: &str,
        to: &str,
        args: &OpCopy,
        max_bytes_rewritten_per_call: Option<usize>,
        rewrite_token: Option<&str>,
    ) -> Result<Request<Buffer>> {
        let source = build_abs_path(&self.root, from);
        let dest = build_abs_path(&self.root, to);

        let url = format!(
            "{}/storage/v1/b/{}/o/{}/rewriteTo/b/{}/o/{}",
            self.endpoint,
            self.bucket,
            gcs_percent_encode_path(&source),
            self.bucket,
            gcs_percent_encode_path(&dest)
        );

        let mut url = QueryPairsWriter::new(&url);

        if let Some(version) = args.source_version() {
            url = url.push("sourceGeneration", &gcs_percent_encode_path(version));
        }
        if let Some(version) = args.if_version_match() {
            url = url.push("ifGenerationMatch", &gcs_percent_encode_path(version));
        } else if args.if_not_exists() {
            url = url.push("ifGenerationMatch", "0");
        }
        if let Some(version) = args.if_version_not_match() {
            url = url.push("ifGenerationNotMatch", &gcs_percent_encode_path(version));
        }
        if let Some(max_bytes) = max_bytes_rewritten_per_call {
            url = url.push("maxBytesRewrittenPerCall", &max_bytes.to_string());
        }
        if let Some(token) = rewrite_token {
            url = url.push("rewriteToken", &gcs_percent_encode_path(token));
        }

        Request::post(url.finish())
            .header(CONTENT_LENGTH, 0)
            .extension(Operation::Copy)
            .extension(ServiceOperation("RewriteObject"))
            .body(Buffer::new())
            .map_err(new_request_build_error)
    }

    pub async fn gcs_rewrite_object(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: &OpCopy,
        max_bytes_rewritten_per_call: Option<usize>,
        rewrite_token: Option<&str>,
    ) -> Result<Response<Buffer>> {
        let req = self.gcs_rewrite_object_request(
            from,
            to,
            args,
            max_bytes_rewritten_per_call,
            rewrite_token,
        )?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn gcs_list_objects(
        &self,
        ctx: &OperationContext,
        path: &str,
        page_token: &str,
        delimiter: &str,
        limit: Option<usize>,
        start_after: Option<String>,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/storage/v1/b/{}/o", self.endpoint, self.bucket,);

        let mut url = QueryPairsWriter::new(&url);
        url = url.push("prefix", &gcs_percent_encode_path(&p));

        if !delimiter.is_empty() {
            url = url.push("delimiter", delimiter);
        }
        if let Some(limit) = limit {
            url = url.push("maxResults", &limit.to_string());
        }
        if let Some(start_after) = start_after {
            let start_after = build_abs_path(&self.root, &start_after);
            url = url.push("startOffset", &gcs_percent_encode_path(&start_after));
        }

        if !page_token.is_empty() {
            // NOTE:
            //
            // GCS uses pageToken in request and nextPageToken in response
            //
            // Don't know how will those tokens be like so this part are copied
            // directly from AWS S3 service.
            url = url.push("pageToken", &gcs_percent_encode_path(page_token));
        }

        let req = Request::get(url.finish())
            .extension(Operation::List)
            .extension(ServiceOperation("ListObjects"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let req = self.sign(ctx, req).await?;

        self.send(ctx, req).await
    }

    pub async fn gcs_initiate_multipart_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        op: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}/{}?uploads",
            self.endpoint,
            self.bucket,
            gcs_percent_encode_path(&p)
        );

        let mut builder = Request::post(&url)
            .header(CONTENT_LENGTH, 0)
            .extension(Operation::Write)
            .extension(ServiceOperation("CreateMultipartUpload"));

        if let Some(header_val) = op.content_disposition() {
            builder = builder.header(CONTENT_DISPOSITION, header_val);
        }

        if let Some(header_val) = op.content_encoding() {
            builder = builder.header(CONTENT_ENCODING, header_val);
        }

        if let Some(header_val) = op.content_type() {
            builder = builder.header(CONTENT_TYPE, header_val);
        }

        if let Some(header_val) = op.cache_control() {
            builder = builder.header(CACHE_CONTROL, header_val);
        }

        if let Some(metadata) = op.user_metadata() {
            for (k, v) in metadata {
                builder = builder.header(&format!("x-goog-meta-{k}"), v);
            }
        }

        if let Some(acl) = self.predefined_acl.as_ref() {
            if let Some(predefined_acl_in_xml_spec) = predefined_acl_to_xml_header(acl) {
                builder = builder.header(X_GOOG_ACL, predefined_acl_in_xml_spec);
            } else {
                log::warn!("Unrecognized predefined_acl. Ignoring");
            }
        }

        let req = builder
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn gcs_upload_part(
        &self,
        ctx: &OperationContext,
        path: &str,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}/{}?partNumber={}&uploadId={}",
            self.endpoint,
            self.bucket,
            gcs_percent_encode_path(&p),
            part_number,
            gcs_percent_encode_path(upload_id)
        );

        let mut req = Request::put(&url);

        req = req.header(CONTENT_LENGTH, size);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadPart"));

        let req = req.body(body).map_err(new_request_build_error)?;

        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn gcs_complete_multipart_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        upload_id: &str,
        parts: Vec<CompleteMultipartUploadRequestPart>,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}/{}?uploadId={}",
            self.endpoint,
            self.bucket,
            gcs_percent_encode_path(&p),
            gcs_percent_encode_path(upload_id)
        );

        let req = Request::post(&url);

        let content = quick_xml::se::to_string(&CompleteMultipartUploadRequest { part: parts })
            .map_err(new_xml_serialize_error)?;
        // Make sure content length has been set to avoid post with chunked encoding.
        let req = req.header(CONTENT_LENGTH, content.len());
        // Set content-type to `application/xml` to avoid mixed with form post.
        let req = req.header(CONTENT_TYPE, "application/xml");

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("CompleteMultipartUpload"));

        let req = req
            .body(Buffer::from(Bytes::from(content)))
            .map_err(new_request_build_error)?;

        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn gcs_abort_multipart_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        upload_id: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}/{}?uploadId={}",
            self.endpoint,
            self.bucket,
            gcs_percent_encode_path(&p),
            gcs_percent_encode_path(upload_id)
        );

        let req = Request::delete(&url)
            .extension(Operation::Write)
            .extension(ServiceOperation("AbortMultipartUpload"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub fn build_metadata_from_object_response(path: &str, data: Buffer) -> Result<Metadata> {
        let meta: GetObjectJsonResponse =
            serde_json::from_reader(data.reader()).map_err(new_json_deserialize_error)?;

        meta.into_metadata(path)
    }
}

impl GetObjectJsonResponse {
    fn into_metadata(self, path: &str) -> Result<Metadata> {
        let size = self
            .size
            .parse::<u64>()
            .map_err(|e| Error::new(ErrorKind::Unexpected, "parse u64").set_source(e))?;
        let mut m = if path.ends_with('/') {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::file(size)
        };

        m.etag(&self.etag);
        m.content_md5(&self.md5_hash);
        if !self.content_type.is_empty() {
            m.content_type(&self.content_type);
        }

        if !self.content_encoding.is_empty() {
            m.content_encoding(&self.content_encoding);
        }

        if !self.cache_control.is_empty() {
            m.cache_control(&self.cache_control);
        }

        if !self.content_disposition.is_empty() {
            m.content_disposition(&self.content_disposition);
        }

        if !self.generation.is_empty() {
            m.version(&self.generation);
        }

        if !self.updated.is_empty() {
            m.last_modified(self.updated.parse::<Timestamp>()?);
        }

        if !self.metadata.is_empty() {
            m.user_metadata(self.metadata);
        }

        Ok(m.build())
    }
}

// https://cloud.google.com/storage/docs/xml-api/reference-headers#xgoogacl
fn predefined_acl_to_xml_header(predefined_acl: &str) -> Option<&'static str> {
    match predefined_acl {
        "projectPrivate" => Some("project-private"),
        "private" => Some("private"),
        "bucketOwnerRead" => Some("bucket-owner-read"),
        "bucketOwnerFullControl" => Some("bucket-owner-full-control"),
        "publicRead" => Some("public-read"),
        "authenticatedRead" => Some("authenticated-read"),
        _ => None,
    }
}

#[derive(Debug, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InsertRequestMetadata<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_encoding: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    storage_class: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<UserMetadata<'a>>,
}

impl InsertRequestMetadata<'_> {
    pub fn is_empty(&self) -> bool {
        self.content_type.is_none()
            && self.content_encoding.is_none()
            && self.storage_class.is_none()
            && self.cache_control.is_none()
            // We could also put content-encoding in the url parameters
            && self.content_encoding.is_none()
            && self.metadata.is_none()
    }
}

#[derive(Clone, Debug)]
pub struct GcsComposeSource {
    pub path: String,
    pub version: Option<String>,
    pub if_version_match: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComposeRequest<'a> {
    source_objects: Vec<ComposeSourceObject>,
    destination: ComposeDestination<'a>,
    delete_source_objects: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComposeSourceObject {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    object_preconditions: Option<ComposeSourceObjectPreconditions>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComposeSourceObjectPreconditions {
    if_generation_match: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComposeDestination<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    storage_class: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_disposition: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_encoding: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<UserMetadata<'a>>,
}
/// Response JSON from GCS list objects API.
///
/// refer to https://cloud.google.com/storage/docs/json_api/v1/objects/list for details
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ListResponse {
    /// The continuation token.
    ///
    /// If this is the last page of results, then no continuation token is returned.
    pub next_page_token: Option<String>,
    /// Object name prefixes for objects that matched the listing request
    /// but were excluded from [items] because of a delimiter.
    pub prefixes: Vec<String>,
    /// The list of objects, ordered lexicographically by name.
    pub items: Vec<ListResponseItem>,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ListResponseItem {
    pub name: String,
    pub size: String,
    // metadata
    pub etag: String,
    pub generation: String,
    pub md5_hash: String,
    pub updated: String,
    pub content_type: String,
}

/// Result of CreateMultipartUpload
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct InitiateMultipartUploadResult {
    pub upload_id: String,
}

/// Request of CompleteMultipartUploadRequest
#[derive(Default, Debug, Serialize)]
#[serde(default, rename = "CompleteMultipartUpload", rename_all = "PascalCase")]
pub struct CompleteMultipartUploadRequest {
    pub part: Vec<CompleteMultipartUploadRequestPart>,
}

#[derive(Clone, Default, Debug, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct CompleteMultipartUploadRequestPart {
    #[serde(rename = "PartNumber")]
    pub part_number: usize,
    #[serde(rename = "ETag")]
    pub etag: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RewriteResponse {
    pub total_bytes_rewritten: String,
    pub done: bool,
    pub rewrite_token: Option<String>,
    resource: Option<GetObjectJsonResponse>,
}

impl RewriteResponse {
    pub fn into_metadata(self, path: &str) -> Result<Metadata> {
        match self.resource {
            Some(resource) => resource.into_metadata(path),
            None => Ok(MetadataBuilder::unknown().build()),
        }
    }
}

/// The raw json response returned by [`get`](https://cloud.google.com/storage/docs/json_api/v1/objects/get)
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct GetObjectJsonResponse {
    /// GCS will return size in string.
    ///
    /// For example: `"size": "56535"`
    size: String,
    /// etag is not quoted.
    ///
    /// For example: `"etag": "CKWasoTgyPkCEAE="`
    etag: String,
    /// RFC3339 styled datetime string.
    ///
    /// For example: `"updated": "2022-08-15T11:33:34.866Z"`
    updated: String,
    /// Content md5 hash
    ///
    /// For example: `"md5Hash": "fHcEH1vPwA6eTPqxuasXcg=="`
    md5_hash: String,
    /// Content type of this object.
    ///
    /// For example: `"contentType": "image/png",`
    content_type: String,
    /// Content encoding of this object
    ///
    /// For example: "contentEncoding": "br"
    content_encoding: String,
    /// Content disposition of this object
    content_disposition: String,
    /// Cache-Control directive for the object data.
    cache_control: String,
    /// Content generation of this object. Used for object versioning and soft delete.
    generation: String,
    /// Custom metadata of this object.
    ///
    /// For example: `"metadata" : { "my-key": "my-value" }`
    metadata: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqsign_core::ProvideCredentialChain;
    use reqsign_google::RequestSigner;
    use reqsign_google::TokenCredentialProvider;

    fn test_core() -> GcsCore {
        let sign_ctx = Context::new();
        let signer = Signer::new(
            sign_ctx.clone(),
            ProvideCredentialChain::new().push(TokenCredentialProvider::new("test-token")),
            RequestSigner::new("storage"),
        );

        GcsCore {
            info: ServiceInfo::new("gcs", "/", "test-bucket"),
            capability: Capability::default(),
            endpoint: "https://storage.googleapis.com".to_string(),
            bucket: "test-bucket".to_string(),
            root: "/".to_string(),
            signer,
            sign_ctx,
            predefined_acl: None,
            default_storage_class: None,
            skip_signature: false,
        }
    }

    fn read_args(options: options::ReadOptions) -> OpRead {
        let (_, args, _) = options.into();
        args
    }

    fn write_args(options: options::WriteOptions) -> OpWrite {
        let (args, _) = OpWrite::from_options(&Capability::default(), options).unwrap();
        args
    }

    fn copy_args(options: options::CopyOptions) -> OpCopy {
        OpCopy::from_options(&Capability::default(), options).unwrap()
    }

    #[tokio::test]
    async fn test_insert_object_signing_preserves_wire_uri() {
        let core = test_core();
        let req = core
            .gcs_insert_object_request(
                "nested/object #1.txt",
                Some(0),
                &OpWrite::default(),
                Buffer::new(),
            )
            .expect("request must build");
        let original_uri = req.uri().clone();

        assert!(
            original_uri
                .to_string()
                .contains("nested%2Fobject%20%231.txt")
        );
        assert!(!original_uri.to_string().contains("%252F"));

        let signed = core
            .sign(&OperationContext::default(), req)
            .await
            .expect("request must sign");

        assert_eq!(signed.uri(), &original_uri);
    }

    #[test]
    fn test_generation_preconditions_are_mapped_to_json_api() {
        let core = test_core();

        let read = core
            .gcs_get_object_request(
                "object",
                BytesRange::default(),
                &read_args(options::ReadOptions {
                    if_version_match: Some("123".to_owned()),
                    ..Default::default()
                }),
            )
            .expect("read request must build");
        assert!(
            read.uri()
                .query()
                .unwrap()
                .contains("ifGenerationMatch=123")
        );

        let read_zero = core
            .gcs_get_object_request(
                "object",
                BytesRange::default(),
                &read_args(options::ReadOptions {
                    if_version_match: Some("0".to_owned()),
                    ..Default::default()
                }),
            )
            .expect("generation zero must be forwarded");
        assert!(
            read_zero
                .uri()
                .query()
                .unwrap()
                .contains("ifGenerationMatch=0")
        );

        let stat = core
            .gcs_head_object_request(
                "object",
                &options::StatOptions {
                    if_version_not_match: Some("456".to_owned()),
                    ..Default::default()
                }
                .into(),
            )
            .expect("stat request must build");
        assert!(
            stat.uri()
                .query()
                .unwrap()
                .contains("ifGenerationNotMatch=456")
        );

        let write = core
            .gcs_insert_object_request(
                "object",
                Some(0),
                &write_args(options::WriteOptions {
                    if_not_exists: true,
                    if_version_match: Some("123".to_owned()),
                    ..Default::default()
                }),
                Buffer::new(),
            )
            .expect("write request must build");
        let query = write.uri().query().unwrap();
        assert!(query.contains("ifGenerationMatch=123"));
        assert_eq!(query.matches("ifGenerationMatch=").count(), 1);

        let delete = core
            .gcs_delete_object_request(
                "object",
                &OpDelete::from_options(
                    &Capability::default(),
                    options::DeleteOptions {
                        if_version_not_match: Some("456".to_owned()),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .expect("delete request must build");
        assert!(
            delete
                .uri()
                .query()
                .unwrap()
                .contains("ifGenerationNotMatch=456")
        );

        let resumable = core
            .gcs_initiate_resumable_upload_request(
                "object",
                &write_args(options::WriteOptions {
                    if_not_exists: true,
                    if_version_match: Some("123".to_owned()),
                    ..Default::default()
                }),
            )
            .expect("resumable request must build");
        let query = resumable.uri().query().unwrap();
        assert!(query.contains("ifGenerationMatch=123"));
        assert_eq!(query.matches("ifGenerationMatch=").count(), 1);

        let rewrite = core
            .gcs_rewrite_object_request(
                "source",
                "target",
                &copy_args(options::CopyOptions {
                    if_version_not_match: Some("456".to_owned()),
                    ..Default::default()
                }),
                Some(GCS_REWRITE_MIN_CHUNK_SIZE),
                Some("token"),
            )
            .expect("rewrite request must build");
        let query = rewrite.uri().query().unwrap();
        assert!(query.contains("ifGenerationNotMatch=456"));
        assert!(query.contains("rewriteToken=token"));

        let rewrite = core
            .gcs_rewrite_object_request(
                "source",
                "target",
                &copy_args(options::CopyOptions {
                    if_not_exists: true,
                    if_version_match: Some("123".to_owned()),
                    ..Default::default()
                }),
                None,
                None,
            )
            .expect("rewrite request must build");
        let query = rewrite.uri().query().unwrap();
        assert!(query.contains("ifGenerationMatch=123"));
        assert_eq!(query.matches("ifGenerationMatch=").count(), 1);
    }

    #[test]
    fn test_xml_requests_with_version() {
        let core = test_core();

        let read = core
            .gcs_get_object_xml_request(
                "object",
                BytesRange::default(),
                &read_args(options::ReadOptions {
                    version: Some("123".to_owned()),
                    override_content_type: Some("text/plain".to_string()),
                    ..Default::default()
                }),
            )
            .expect("read request must build");
        assert_eq!(read.method(), http::Method::GET);
        assert_eq!(
            read.uri().to_string(),
            "https://storage.googleapis.com/test-bucket/object?generation=123&response-content-type=text/plain"
        );

        let stat = core
            .gcs_head_object_xml_request(
                "object",
                &options::StatOptions {
                    version: Some("123".to_owned()),
                    ..Default::default()
                }
                .into(),
            )
            .expect("stat request must build");
        assert_eq!(stat.method(), http::Method::HEAD);
        assert_eq!(
            stat.uri().to_string(),
            "https://storage.googleapis.com/test-bucket/object?generation=123"
        );
    }

    #[test]
    fn test_get_object_request_with_version() {
        let core = test_core();

        let req = core
            .gcs_get_object_request(
                "object",
                BytesRange::default(),
                &read_args(options::ReadOptions {
                    version: Some("123".to_owned()),
                    if_version_match: Some("456".to_owned()),
                    ..Default::default()
                }),
            )
            .expect("read request must build");
        assert_eq!(req.method(), http::Method::GET);
        assert_eq!(
            req.uri().to_string(),
            "https://storage.googleapis.com/storage/v1/b/test-bucket/o/object?alt=media&generation=123&ifGenerationMatch=456"
        );
    }

    #[test]
    fn test_rewrite_object_request_with_source_version() {
        let core = test_core();

        let req = core
            .gcs_rewrite_object_request(
                "source",
                "target",
                &copy_args(options::CopyOptions {
                    source_version: Some("123".to_owned()),
                    ..Default::default()
                }),
                None,
                Some("token"),
            )
            .expect("rewrite request must build");
        assert_eq!(req.method(), http::Method::POST);
        assert_eq!(
            req.uri().to_string(),
            "https://storage.googleapis.com/storage/v1/b/test-bucket/o/source/rewriteTo/b/test-bucket/o/target?sourceGeneration=123&rewriteToken=token"
        );
    }

    #[test]
    fn test_head_object_request_with_version() {
        let core = test_core();

        let req = core
            .gcs_head_object_request(
                "object",
                &options::StatOptions {
                    version: Some("123".to_owned()),
                    if_version_match: Some("456".to_owned()),
                    ..Default::default()
                }
                .into(),
            )
            .expect("stat request must build");
        assert_eq!(req.method(), http::Method::GET);
        assert_eq!(
            req.uri().to_string(),
            "https://storage.googleapis.com/storage/v1/b/test-bucket/o/object?generation=123&ifGenerationMatch=456"
        );
    }

    fn delete_args(options: options::DeleteOptions) -> OpDelete {
        OpDelete::from_options(&Capability::default(), options).unwrap()
    }

    #[test]
    fn test_delete_object_request_with_version() {
        let core = test_core();

        let req = core
            .gcs_delete_object_request(
                "object",
                &delete_args(options::DeleteOptions {
                    version: Some("123".to_owned()),
                    ..Default::default()
                }),
            )
            .expect("delete request must build");
        assert_eq!(req.method(), http::Method::DELETE);
        assert_eq!(
            req.uri().to_string(),
            "https://storage.googleapis.com/storage/v1/b/test-bucket/o/object?generation=123"
        );

        let req = core
            .gcs_delete_object_request(
                "object",
                &delete_args(options::DeleteOptions {
                    version: Some("123".to_owned()),
                    if_version_match: Some("456".to_owned()),
                    ..Default::default()
                }),
            )
            .expect("delete request must build");
        assert_eq!(
            req.uri().query(),
            Some("generation=123&ifGenerationMatch=456")
        );
    }

    #[test]
    fn test_delete_objects_request_with_version() {
        let core = test_core();
        let batch = vec![
            (
                "one".to_string(),
                delete_args(options::DeleteOptions {
                    version: Some("123".to_owned()),
                    ..Default::default()
                }),
            ),
            ("two".to_string(), OpDelete::new()),
        ];

        let req = core
            .gcs_delete_objects_request(&batch)
            .expect("batch delete request must build");
        let body = String::from_utf8(req.body().to_bytes().to_vec()).unwrap();

        assert!(body.contains("DELETE /storage/v1/b/test-bucket/o/one?generation=123 HTTP/1.1"));
        assert!(body.contains("DELETE /storage/v1/b/test-bucket/o/two HTTP/1.1"));
        assert_eq!(body.matches("generation=").count(), 1);
    }

    #[test]
    fn test_compose_object_request() {
        let mut core = test_core();
        core.default_storage_class = Some("NEARLINE".to_string());
        let sources = [
            GcsComposeSource {
                path: "source/one".to_string(),
                version: Some("11".to_string()),
                if_version_match: None,
            },
            GcsComposeSource {
                path: "source two".to_string(),
                version: None,
                if_version_match: Some("22".to_string()),
            },
        ];
        let args = OpCompose::from_options(
            &Capability::default(),
            options::ComposeOptions {
                if_version_match: Some("33".to_string()),
                cache_control: Some("no-cache".to_string()),
                content_type: Some("text/plain".to_string()),
                content_disposition: Some("attachment".to_string()),
                content_encoding: Some("gzip".to_string()),
                user_metadata: Some(HashMap::from([("key".to_string(), "value".to_string())])),
                ..Default::default()
            },
        )
        .unwrap();

        let req = core
            .gcs_compose_object_request(&sources, "target/object #1", &args)
            .expect("compose request must build");

        assert_eq!(req.method(), http::Method::POST);
        assert!(
            req.uri()
                .path()
                .ends_with("/o/target%2Fobject%20%231/compose")
        );
        let query = req.uri().query().expect("compose query must exist");
        assert!(query.contains("ifGenerationMatch=33"));

        let body: serde_json::Value = serde_json::from_slice(&req.body().to_bytes())
            .expect("compose body must be valid JSON");
        assert_eq!(
            body,
            serde_json::json!({
                "sourceObjects": [
                    {"name": "source/one", "generation": "11"},
                    {
                        "name": "source two",
                        "objectPreconditions": {"ifGenerationMatch": "22"}
                    }
                ],
                "destination": {
                    "storageClass": "NEARLINE",
                    "cacheControl": "no-cache",
                    "contentType": "text/plain",
                    "contentDisposition": "attachment",
                    "contentEncoding": "gzip",
                    "metadata": {"key": "value"}
                },
                "deleteSourceObjects": false
            })
        );
    }

    #[test]
    fn test_generation_match_is_mapped_to_xml_api() {
        let core = test_core();

        let read = core
            .gcs_get_object_xml_request(
                "object",
                BytesRange::default(),
                &read_args(options::ReadOptions {
                    if_version_match: Some("123".to_owned()),
                    ..Default::default()
                }),
            )
            .expect("read request must build");
        assert_eq!(read.headers()["x-goog-if-generation-match"], "123");

        let stat = core
            .gcs_head_object_xml_request(
                "object",
                &options::StatOptions {
                    if_version_match: Some("123".to_owned()),
                    ..Default::default()
                }
                .into(),
            )
            .expect("stat request must build");
        assert_eq!(stat.headers()["x-goog-if-generation-match"], "123");

        let write = core
            .gcs_insert_object_xml_request(
                "object",
                &write_args(options::WriteOptions {
                    if_not_exists: true,
                    if_version_match: Some("123".to_owned()),
                    ..Default::default()
                }),
                Buffer::new(),
            )
            .expect("write request must build");
        assert_eq!(write.headers()["x-goog-if-generation-match"], "123");
        assert_eq!(
            write
                .headers()
                .get_all("x-goog-if-generation-match")
                .iter()
                .count(),
            1
        );

        let create = core
            .gcs_insert_object_xml_request(
                "object",
                &write_args(options::WriteOptions {
                    if_not_exists: true,
                    ..Default::default()
                }),
                Buffer::new(),
            )
            .expect("create request must build");
        assert_eq!(create.headers()["x-goog-if-generation-match"], "0");
    }

    #[test]
    fn test_gcs_get_object_xml_request_with_response_overrides() {
        let core = test_core();

        let read = core
            .gcs_get_object_xml_request(
                "object",
                BytesRange::default(),
                &read_args(options::ReadOptions {
                    override_content_disposition: Some(
                        "attachment; filename=\"other.txt\"".to_string(),
                    ),
                    override_content_type: Some("application/pdf".to_string()),
                    override_cache_control: Some("max-age=3600".to_string()),
                    ..Default::default()
                }),
            )
            .expect("read request must build");

        let query = read.uri().query().unwrap();
        assert!(
            query.contains(
                "response-content-disposition=attachment%3B%20filename%3D%22other.txt%22"
            )
        );
        assert!(query.contains("response-content-type=application/pdf"));
        assert!(query.contains("response-cache-control=max-age%3D3600"));
    }

    #[test]
    fn test_gcs_head_object_xml_request_with_response_overrides() {
        let core = test_core();

        let stat = core
            .gcs_head_object_xml_request(
                "object",
                &options::StatOptions {
                    override_content_disposition: Some(
                        "attachment; filename=\"other.txt\"".to_string(),
                    ),
                    override_content_type: Some("application/pdf".to_string()),
                    override_cache_control: Some("max-age=3600".to_string()),
                    ..Default::default()
                }
                .into(),
            )
            .expect("stat request must build");

        let query = stat.uri().query().unwrap();
        assert!(
            query.contains(
                "response-content-disposition=attachment%3B%20filename%3D%22other.txt%22"
            )
        );
        assert!(query.contains("response-content-type=application/pdf"));
        assert!(query.contains("response-cache-control=max-age%3D3600"));
    }

    #[test]
    fn test_parse_missing_target_with_version_condition() {
        let parse_missing = |ctx| {
            let resp = Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Buffer::new())
                .expect("response must build");
            parse_error(ctx, resp).kind()
        };

        let read_ctx = ErrorContext::new(ServiceOperation("GetObject")).with_caller_condition(true);
        assert_eq!(parse_missing(read_ctx), ErrorKind::NotFound);

        let delete_ctx =
            ErrorContext::new(ServiceOperation("DeleteObject")).with_delete_match_condition(true);
        assert_eq!(parse_missing(delete_ctx), ErrorKind::ConditionNotMatch);
    }

    #[test]
    fn test_deserialize_get_object_json_response() {
        let content = r#"{
    "kind": "storage#object",
    "id": "example/1.png/1660563214863653",
    "selfLink": "https://www.googleapis.com/storage/v1/b/example/o/1.png",
    "mediaLink": "https://content-storage.googleapis.com/download/storage/v1/b/example/o/1.png?generation=1660563214863653&alt=media",
    "name": "1.png",
    "bucket": "example",
    "generation": "1660563214863653",
    "metageneration": "1",
    "contentType": "image/png",
    "contentEncoding": "br",
    "contentDisposition": "attachment",
    "cacheControl": "public, max-age=3600",
    "storageClass": "STANDARD",
    "size": "56535",
    "md5Hash": "fHcEH1vPwA6eTPqxuasXcg==",
    "crc32c": "j/un9g==",
    "etag": "CKWasoTgyPkCEAE=",
    "timeCreated": "2022-08-15T11:33:34.866Z",
    "updated": "2022-08-15T11:33:34.866Z",
    "timeStorageClassUpdated": "2022-08-15T11:33:34.866Z",
    "metadata" : {
        "location" : "everywhere"
  }
}"#;

        let meta = GcsCore::build_metadata_from_object_response("1.png", content.into())
            .expect("parse metadata should not fail");

        assert_eq!(meta.content_length(), 56535);
        assert_eq!(
            meta.last_modified(),
            Some(
                "2022-08-15T11:33:34.866Z"
                    .parse::<Timestamp>()
                    .expect("parse date should not fail")
            )
        );
        assert_eq!(meta.content_md5(), Some("fHcEH1vPwA6eTPqxuasXcg=="));
        assert_eq!(meta.etag(), Some("CKWasoTgyPkCEAE="));
        assert_eq!(meta.content_type(), Some("image/png"));
        assert_eq!(meta.content_encoding(), Some("br"));
        assert_eq!(meta.content_disposition(), Some("attachment"));
        assert_eq!(meta.cache_control(), Some("public, max-age=3600"));
        assert_eq!(meta.version(), Some("1660563214863653"));

        let metadata = HashMap::from_iter([("location".to_string(), "everywhere".to_string())]);
        assert_eq!(
            meta.user_metadata()
                .expect("user metadata must be present")
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect::<HashMap<_, _>>(),
            metadata
        );
    }

    #[test]
    fn test_deserialize_list_response() {
        let content = r#"
    {
  "kind": "storage#objects",
  "prefixes": [
    "dir/",
    "test/"
  ],
  "items": [
    {
      "kind": "storage#object",
      "id": "example/1.png/1660563214863653",
      "selfLink": "https://www.googleapis.com/storage/v1/b/example/o/1.png",
      "mediaLink": "https://content-storage.googleapis.com/download/storage/v1/b/example/o/1.png?generation=1660563214863653&alt=media",
      "name": "1.png",
      "bucket": "example",
      "generation": "1660563214863653",
      "metageneration": "1",
      "contentType": "image/png",
      "storageClass": "STANDARD",
      "size": "56535",
      "md5Hash": "fHcEH1vPwA6eTPqxuasXcg==",
      "crc32c": "j/un9g==",
      "etag": "CKWasoTgyPkCEAE=",
      "timeCreated": "2022-08-15T11:33:34.866Z",
      "updated": "2022-08-15T11:33:34.866Z",
      "timeStorageClassUpdated": "2022-08-15T11:33:34.866Z"
    },
    {
      "kind": "storage#object",
      "id": "example/2.png/1660563214883337",
      "selfLink": "https://www.googleapis.com/storage/v1/b/example/o/2.png",
      "mediaLink": "https://content-storage.googleapis.com/download/storage/v1/b/example/o/2.png?generation=1660563214883337&alt=media",
      "name": "2.png",
      "bucket": "example",
      "generation": "1660563214883337",
      "metageneration": "1",
      "contentType": "image/png",
      "storageClass": "STANDARD",
      "size": "45506",
      "md5Hash": "e6LsGusU7pFJZk+114NV1g==",
      "crc32c": "L00QAg==",
      "etag": "CIm0s4TgyPkCEAE=",
      "timeCreated": "2022-08-15T11:33:34.886Z",
      "updated": "2022-08-15T11:33:34.886Z",
      "timeStorageClassUpdated": "2022-08-15T11:33:34.886Z"
    }
  ]
}
    "#;

        let output: ListResponse =
            serde_json::from_str(content).expect("JSON deserialize must succeed");
        assert!(output.next_page_token.is_none());
        assert_eq!(output.items.len(), 2);
        assert_eq!(output.items[0].name, "1.png");
        assert_eq!(output.items[0].size, "56535");
        assert_eq!(output.items[0].md5_hash, "fHcEH1vPwA6eTPqxuasXcg==");
        assert_eq!(output.items[0].etag, "CKWasoTgyPkCEAE=");
        assert_eq!(output.items[0].generation, "1660563214863653");
        assert_eq!(output.items[0].updated, "2022-08-15T11:33:34.866Z");
        assert_eq!(output.items[1].name, "2.png");
        assert_eq!(output.items[1].size, "45506");
        assert_eq!(output.items[1].md5_hash, "e6LsGusU7pFJZk+114NV1g==");
        assert_eq!(output.items[1].etag, "CIm0s4TgyPkCEAE=");
        assert_eq!(output.items[1].updated, "2022-08-15T11:33:34.886Z");
        assert_eq!(output.items[1].content_type, "image/png");
        assert_eq!(output.prefixes, vec!["dir/", "test/"])
    }

    #[test]
    fn test_deserialize_list_response_with_next_page_token() {
        let content = r#"
    {
  "kind": "storage#objects",
  "prefixes": [
    "dir/",
    "test/"
  ],
  "nextPageToken": "CgYxMC5wbmc=",
  "items": [
    {
      "kind": "storage#object",
      "id": "example/1.png/1660563214863653",
      "selfLink": "https://www.googleapis.com/storage/v1/b/example/o/1.png",
      "mediaLink": "https://content-storage.googleapis.com/download/storage/v1/b/example/o/1.png?generation=1660563214863653&alt=media",
      "name": "1.png",
      "bucket": "example",
      "generation": "1660563214863653",
      "metageneration": "1",
      "contentType": "image/png",
      "storageClass": "STANDARD",
      "size": "56535",
      "md5Hash": "fHcEH1vPwA6eTPqxuasXcg==",
      "crc32c": "j/un9g==",
      "etag": "CKWasoTgyPkCEAE=",
      "timeCreated": "2022-08-15T11:33:34.866Z",
      "updated": "2022-08-15T11:33:34.866Z",
      "timeStorageClassUpdated": "2022-08-15T11:33:34.866Z"
    },
    {
      "kind": "storage#object",
      "id": "example/2.png/1660563214883337",
      "selfLink": "https://www.googleapis.com/storage/v1/b/example/o/2.png",
      "mediaLink": "https://content-storage.googleapis.com/download/storage/v1/b/example/o/2.png?generation=1660563214883337&alt=media",
      "name": "2.png",
      "bucket": "example",
      "generation": "1660563214883337",
      "metageneration": "1",
      "contentType": "image/png",
      "storageClass": "STANDARD",
      "size": "45506",
      "md5Hash": "e6LsGusU7pFJZk+114NV1g==",
      "crc32c": "L00QAg==",
      "etag": "CIm0s4TgyPkCEAE=",
      "timeCreated": "2022-08-15T11:33:34.886Z",
      "updated": "2022-08-15T11:33:34.886Z",
      "timeStorageClassUpdated": "2022-08-15T11:33:34.886Z"
    }
  ]
}
    "#;

        let output: ListResponse =
            serde_json::from_str(content).expect("JSON deserialize must succeed");
        assert_eq!(output.next_page_token, Some("CgYxMC5wbmc=".to_string()));
        assert_eq!(output.items.len(), 2);
        assert_eq!(output.items[0].name, "1.png");
        assert_eq!(output.items[0].size, "56535");
        assert_eq!(output.items[0].md5_hash, "fHcEH1vPwA6eTPqxuasXcg==");
        assert_eq!(output.items[0].etag, "CKWasoTgyPkCEAE=");
        assert_eq!(output.items[0].updated, "2022-08-15T11:33:34.866Z");
        assert_eq!(output.items[1].name, "2.png");
        assert_eq!(output.items[1].size, "45506");
        assert_eq!(output.items[1].md5_hash, "e6LsGusU7pFJZk+114NV1g==");
        assert_eq!(output.items[1].etag, "CIm0s4TgyPkCEAE=");
        assert_eq!(output.items[1].updated, "2022-08-15T11:33:34.886Z");
        assert_eq!(output.prefixes, vec!["dir/", "test/"])
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ErrorContext {
    service_operation: ServiceOperation,
    caller_condition: bool,
    delete_match_condition: bool,
    internal_condition: bool,
}

impl ErrorContext {
    pub const fn new(service_operation: ServiceOperation) -> Self {
        Self {
            service_operation,
            caller_condition: false,
            delete_match_condition: false,
            internal_condition: false,
        }
    }

    pub const fn with_caller_condition(mut self, caller_condition: bool) -> Self {
        self.caller_condition = caller_condition;
        self
    }

    pub const fn with_delete_match_condition(mut self, delete_match_condition: bool) -> Self {
        self.caller_condition = self.caller_condition || delete_match_condition;
        self.delete_match_condition = delete_match_condition;
        self
    }

    pub const fn with_internal_condition(mut self, internal_condition: bool) -> Self {
        self.internal_condition = internal_condition;
        self
    }
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct GcsErrorResponse {
    error: GcsError,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct GcsError {
    code: usize,
    message: String,
    errors: Vec<GcsErrorDetail>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct GcsErrorDetail {
    domain: String,
    location: String,
    location_type: String,
    message: String,
    reason: String,
}

/// Parse error response into Error.
pub fn parse_error(ctx: ErrorContext, resp: Response<Buffer>) -> Error {
    let (parts, body) = resp.into_parts();
    let bs = body.to_bytes();

    let gcs_error = de::from_slice::<GcsErrorResponse>(&bs).ok();

    let (mut kind, mut retryable) = match parts.status {
        StatusCode::NOT_FOUND if ctx.delete_match_condition => {
            (ErrorKind::ConditionNotMatch, false)
        }
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        StatusCode::NOT_MODIFIED | StatusCode::PRECONDITION_FAILED if ctx.internal_condition => {
            (ErrorKind::Conflict, false)
        }
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

    if let Some(gcs_error) = &gcs_error {
        let has_reason = gcs_error
            .error
            .errors
            .iter()
            .any(|detail| !detail.reason.is_empty());
        if gcs_error
            .error
            .errors
            .iter()
            .any(|detail| detail.reason == "conditionNotMet")
        {
            if ctx.internal_condition {
                (kind, retryable) = (ErrorKind::Conflict, false);
            } else if ctx.caller_condition {
                (kind, retryable) = (ErrorKind::ConditionNotMatch, false);
            } else {
                (kind, retryable) = (ErrorKind::Unexpected, false);
            }
        } else if gcs_error
            .error
            .errors
            .iter()
            .any(|detail| detail.reason == "conflict")
        {
            (kind, retryable) = (ErrorKind::Conflict, false);
        } else if has_reason
            && matches!(
                parts.status,
                StatusCode::CONFLICT | StatusCode::PRECONDITION_FAILED
            )
        {
            (kind, retryable) = (ErrorKind::Unexpected, false);
        }
    }

    let message = match gcs_error {
        Some(gcs_err) => format!("{gcs_err:?}"),
        None => String::from_utf8_lossy(&bs).into_owned(),
    };

    let mut err =
        Error::new(kind, message).with_context("service_operation", ctx.service_operation.0);

    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}

#[cfg(test)]
mod error_tests {
    use super::*;

    fn condition_not_met(ctx: ErrorContext) -> Error {
        let body = Buffer::from(
            r#"{"error":{"code":412,"message":"condition failed","errors":[{"reason":"conditionNotMet"}]}}"#,
        );
        let resp = Response::builder()
            .status(StatusCode::PRECONDITION_FAILED)
            .body(body)
            .expect("response must build");
        parse_error(ctx, resp)
    }

    #[test]
    fn condition_not_met_uses_operation_context() {
        let caller =
            ErrorContext::new(ServiceOperation("ComposeObject")).with_caller_condition(true);
        assert_eq!(
            condition_not_met(caller).kind(),
            ErrorKind::ConditionNotMatch
        );

        let internal = ErrorContext::new(ServiceOperation("ComposeObject"))
            .with_caller_condition(true)
            .with_internal_condition(true);
        assert_eq!(condition_not_met(internal).kind(), ErrorKind::Conflict);

        let no_condition = ErrorContext::new(ServiceOperation("ComposeObject"));
        assert_eq!(
            condition_not_met(no_condition).kind(),
            ErrorKind::Unexpected
        );
    }
}

mod uri {
    use percent_encoding::AsciiSet;
    use percent_encoding::NON_ALPHANUMERIC;
    use percent_encoding::utf8_percent_encode;

    /// PATH_ENCODE_SET is the encode set for http url path.
    ///
    /// This set follows [encodeURIComponent](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/encodeURIComponent) which will encode all non-ASCII characters except `A-Z a-z 0-9 - _ . ! ~ * ' ( )`
    ///
    /// Following characters is allowed in GCS, check "https://cloud.google.com/storage/docs/request-endpoints#encoding" for details
    static GCS_PATH_ENCODE_SET: AsciiSet = NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'*');

    /// gcs_percent_encode_path will do percent encoding for http encode path.
    ///
    /// Follows [encodeURIComponent](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/encodeURIComponent)
    /// which will encode all non-ASCII characters except `A-Z a-z 0-9 - _ . *`
    ///
    /// GCS does not allow '/'s in paths, this should also be dealt with
    pub(crate) fn gcs_percent_encode_path(path: &str) -> String {
        utf8_percent_encode(path, &GCS_PATH_ENCODE_SET).to_string()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_percent_encode_path() {
            let cases = vec![
                (
                    "Reserved Characters",
                    ";,/?:@&=+$",
                    "%3B%2C%2F%3F%3A%40%26%3D%2B%24",
                ),
                ("Unescaped Characters", "-_.*", "-_.*"),
                ("Number Sign", "#", "%23"),
                (
                    "Alphanumeric Characters + Space",
                    "ABC abc 123",
                    "ABC%20abc%20123",
                ),
                (
                    "Unicode",
                    "你好，世界！❤",
                    "%E4%BD%A0%E5%A5%BD%EF%BC%8C%E4%B8%96%E7%95%8C%EF%BC%81%E2%9D%A4",
                ),
            ];

            for (name, input, expected) in cases {
                let actual = gcs_percent_encode_path(input);

                assert_eq!(actual, expected, "{name}");
            }
        }
    }
}

pub(super) use uri::*;
