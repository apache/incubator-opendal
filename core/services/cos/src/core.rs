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

use bytes::Bytes;
use http::Request;
use http::Response;
use http::header::CACHE_CONTROL;
use http::header::CONTENT_DISPOSITION;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::header::IF_MATCH;
use http::header::IF_MODIFIED_SINCE;
use http::header::IF_NONE_MATCH;
use http::header::IF_UNMODIFIED_SINCE;
use reqsign_core::{Context, Signer};
use reqsign_tencent_cos::Credential;
use serde::Deserialize;
use serde::Serialize;

use opendal_core::Buffer;
use opendal_core::BytesRange;
use opendal_core::Result;
use opendal_core::raw::*;
use opendal_core::*;

pub mod constants {
    pub const COS_QUERY_VERSION_ID: &str = "versionId";

    pub const X_COS_VERSION_ID: &str = "x-cos-version-id";
}

pub struct CosCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub bucket: String,
    pub root: String,
    pub endpoint: String,

    pub signer: Signer<Credential>,
}

impl Debug for CosCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CosCore")
            .field("root", &self.root)
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl CosCore {
    fn signer(&self, ctx: &OperationContext) -> Signer<Credential> {
        self.signer.clone().with_context(
            Context::new()
                .with_file_read(reqsign_file_read_tokio::TokioFileRead)
                .with_http_send(ctx.http_transport().clone())
                .with_env(reqsign_core::OsEnv),
        )
    }

    pub async fn sign<T>(&self, ctx: &OperationContext, req: Request<T>) -> Result<Request<T>> {
        let (mut parts, body) = req.into_parts();

        self.signer(ctx)
            .sign(&mut parts, None)
            .await
            .map_err(|e| new_request_sign_error(e.into()))?;

        Ok(Request::from_parts(parts, body))
    }

    pub async fn sign_query<T>(
        &self,
        ctx: &OperationContext,
        req: Request<T>,
        duration: Duration,
    ) -> Result<Request<T>> {
        let (mut parts, body) = req.into_parts();

        self.signer(ctx)
            .sign(&mut parts, Some(duration))
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
}

impl CosCore {
    pub async fn cos_get_object(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Response<HttpBody>> {
        let req = self.cos_get_object_request(path, range, args)?;
        let req = self.sign(ctx, req).await?;

        ctx.http_transport().fetch(req).await
    }

    pub fn cos_get_object_request(
        &self,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        let mut query_args = Vec::new();
        if let Some(version) = args.version() {
            query_args.push(format!(
                "{}={}",
                constants::COS_QUERY_VERSION_ID,
                percent_decode_path(version)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        let mut req = Request::get(&url);

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }

        if !range.is_full() {
            req = req.header(http::header::RANGE, range.to_header())
        }

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        if let Some(if_modified_since) = args.if_modified_since() {
            req = req.header(IF_MODIFIED_SINCE, if_modified_since.format_http_date());
        }

        if let Some(if_unmodified_since) = args.if_unmodified_since() {
            req = req.header(IF_UNMODIFIED_SINCE, if_unmodified_since.format_http_date());
        }

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("GetObject"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub fn cos_put_object_request(
        &self,
        path: &str,
        size: Option<u64>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        let mut req = Request::put(&url);

        if let Some(size) = size {
            req = req.header(CONTENT_LENGTH, size)
        }
        if let Some(cache_control) = args.cache_control() {
            req = req.header(CACHE_CONTROL, cache_control)
        }
        if let Some(pos) = args.content_disposition() {
            req = req.header(CONTENT_DISPOSITION, pos)
        }
        if let Some(mime) = args.content_type() {
            req = req.header(CONTENT_TYPE, mime)
        }

        // For a bucket which has never enabled versioning, you may use it to
        // specify whether to prohibit overwriting the object with the same name
        // when uploading the object:
        //
        // When the x-cos-forbid-overwrite is specified as true, overwriting the object
        // with the same name will be prohibited.
        //
        // ref: https://www.tencentcloud.com/document/product/436/7749
        if args.if_not_exists() {
            req = req.header("x-cos-forbid-overwrite", "true")
        }

        // Set user metadata headers.
        if let Some(user_metadata) = args.user_metadata() {
            for (key, value) in user_metadata {
                req = req.header(format!("x-cos-meta-{key}"), value)
            }
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("PutObject"));

        let req = req.body(body).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn cos_head_object(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpStat,
    ) -> Result<Response<Buffer>> {
        let req = self.cos_head_object_request(path, args)?;
        let req = self.sign(ctx, req).await?;

        self.send(ctx, req).await
    }

    pub fn cos_head_object_request(&self, path: &str, args: &OpStat) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        let mut query_args = Vec::new();
        if let Some(version) = args.version() {
            query_args.push(format!(
                "{}={}",
                constants::COS_QUERY_VERSION_ID,
                percent_decode_path(version)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        let mut req = Request::head(&url);

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("HeadObject"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn cos_delete_object(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpDelete,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        let mut query_args = Vec::new();
        if let Some(version) = args.version() {
            query_args.push(format!(
                "{}={}",
                constants::COS_QUERY_VERSION_ID,
                percent_decode_path(version)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        let req = Request::delete(&url);

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteObject"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;

        self.send(ctx, req).await
    }

    pub fn cos_append_object_request(
        &self,
        path: &str,
        position: u64,
        size: u64,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);
        let url = format!(
            "{}/{}?append&position={}",
            self.endpoint,
            percent_encode_path(&p),
            position
        );

        let mut req = Request::post(&url);

        req = req.header(CONTENT_LENGTH, size);

        if let Some(mime) = args.content_type() {
            req = req.header(CONTENT_TYPE, mime);
        }

        if let Some(pos) = args.content_disposition() {
            req = req.header(CONTENT_DISPOSITION, pos);
        }

        if let Some(cache_control) = args.cache_control() {
            req = req.header(CACHE_CONTROL, cache_control)
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("AppendObject"));

        let req = req.body(body).map_err(new_request_build_error)?;
        Ok(req)
    }

    pub async fn cos_copy_object(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: &OpCopy,
    ) -> Result<Response<Buffer>> {
        let source = build_abs_path(&self.root, from);
        let target = build_abs_path(&self.root, to);

        let mut source = format!("/{}/{}", self.bucket, percent_encode_path(&source));
        if let Some(version) = args.source_version() {
            source.push_str("?versionId=");
            source.push_str(&percent_encode_path(version));
        }
        let url = format!("{}/{}", self.endpoint, percent_encode_path(&target));

        let mut req = Request::put(&url)
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyObject"))
            .header("x-cos-copy-source", &source);

        // For a bucket which has never enabled versioning, x-cos-forbid-overwrite
        // tells COS to reject the copy when the destination object already exists.
        //
        // ref: https://www.tencentcloud.com/document/product/436/10881
        if args.if_not_exists() {
            req = req.header("x-cos-forbid-overwrite", "true");
        }

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        let req = self.sign(ctx, req).await?;

        self.send(ctx, req).await
    }

    pub async fn cos_list_objects(
        &self,
        ctx: &OperationContext,
        path: &str,
        next_marker: &str,
        delimiter: &str,
        limit: Option<usize>,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = QueryPairsWriter::new(&self.endpoint);

        if !p.is_empty() {
            url = url.push("prefix", &percent_encode_path(&p));
        }
        if !delimiter.is_empty() {
            url = url.push("delimiter", delimiter);
        }
        if let Some(limit) = limit {
            url = url.push("max-keys", &limit.to_string());
        }
        if !next_marker.is_empty() {
            url = url.push("marker", &percent_encode_path(next_marker));
        }

        let req = Request::get(url.finish())
            .extension(Operation::List)
            .extension(ServiceOperation("ListObjects"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let req = self.sign(ctx, req).await?;

        self.send(ctx, req).await
    }

    pub async fn cos_initiate_multipart_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}?uploads", self.endpoint, percent_encode_path(&p));

        let mut req = Request::post(&url);

        if let Some(mime) = args.content_type() {
            req = req.header(CONTENT_TYPE, mime)
        }

        if let Some(content_disposition) = args.content_disposition() {
            req = req.header(CONTENT_DISPOSITION, content_disposition)
        }

        if let Some(cache_control) = args.cache_control() {
            req = req.header(CACHE_CONTROL, cache_control)
        }

        // COS evaluates x-cos-forbid-overwrite on both InitiateMultipartUpload and
        // CompleteMultipartUpload, so a multipart if_not_exists write has to carry it
        // on both requests. Setting it only on the simple PutObject path lets large
        // uploads silently overwrite an existing object.
        //
        // ref: https://www.tencentcloud.com/document/product/436/7746
        if args.if_not_exists() {
            req = req.header("x-cos-forbid-overwrite", "true");
        }

        // Set user metadata headers.
        if let Some(user_metadata) = args.user_metadata() {
            for (key, value) in user_metadata {
                req = req.header(format!("x-cos-meta-{key}"), value)
            }
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("InitiateMultipartUpload"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;

        self.send(ctx, req).await
    }

    pub async fn cos_upload_part_request(
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
            "{}/{}?partNumber={}&uploadId={}",
            self.endpoint,
            percent_encode_path(&p),
            part_number,
            percent_encode_path(upload_id)
        );

        let mut req = Request::put(&url);
        req = req.header(CONTENT_LENGTH, size);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadPart"));

        // Set body
        let req = req.body(body).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;

        self.send(ctx, req).await
    }

    pub async fn cos_complete_multipart_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        upload_id: &str,
        parts: Vec<CompleteMultipartUploadRequestPart>,
        args: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}?uploadId={}",
            self.endpoint,
            percent_encode_path(&p),
            percent_encode_path(upload_id)
        );

        let mut req = Request::post(&url);

        let content = quick_xml::se::to_string(&CompleteMultipartUploadRequest { part: parts })
            .map_err(new_xml_serialize_error)?;
        // Make sure content length has been set to avoid post with chunked encoding.
        req = req.header(CONTENT_LENGTH, content.len());
        // Set content-type to `application/xml` to avoid mixed with form post.
        req = req.header(CONTENT_TYPE, "application/xml");
        // CompleteMultipartUpload is the request that commits the object, so the
        // if_not_exists guard has to be repeated here alongside InitiateMultipartUpload.
        //
        // ref: https://www.tencentcloud.com/document/product/436/7742
        if args.if_not_exists() {
            req = req.header("x-cos-forbid-overwrite", "true");
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("CompleteMultipartUpload"));

        let req = req
            .body(Buffer::from(Bytes::from(content)))
            .map_err(new_request_build_error)?;

        let req = self.sign(ctx, req).await?;

        self.send(ctx, req).await
    }

    /// Abort an on-going multipart upload.
    pub async fn cos_abort_multipart_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        upload_id: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}?uploadId={}",
            self.endpoint,
            percent_encode_path(&p),
            percent_encode_path(upload_id)
        );

        let req = Request::delete(&url)
            .extension(Operation::Delete)
            .extension(ServiceOperation("AbortMultipartUpload"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn cos_list_object_versions(
        &self,
        ctx: &OperationContext,
        prefix: &str,
        delimiter: &str,
        limit: Option<usize>,
        key_marker: &str,
        version_id_marker: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, prefix);

        let mut url = QueryPairsWriter::new(&self.endpoint);
        url = url.push("versions", "");
        if !p.is_empty() {
            url = url.push("prefix", &percent_encode_path(p.as_str()));
        }
        if !delimiter.is_empty() {
            url = url.push("delimiter", delimiter);
        }

        if let Some(limit) = limit {
            url = url.push("max-keys", &limit.to_string());
        }
        if !key_marker.is_empty() {
            url = url.push("key-marker", &percent_encode_path(key_marker));
        }
        if !version_id_marker.is_empty() {
            url = url.push("version-id-marker", &percent_encode_path(version_id_marker));
        }

        let req = Request::get(url.finish())
            .extension(Operation::List)
            .extension(ServiceOperation("ListObjectVersions"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let req = self.sign(ctx, req).await?;

        self.send(ctx, req).await
    }
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
    /// # TODO
    ///
    /// quick-xml will do escape on `"` which leads to our serialized output is
    /// not the same as aws s3's example.
    ///
    /// Ideally, we could use `serialize_with` to address this (buf failed)
    ///
    /// ```ignore
    /// #[derive(Default, Debug, Serialize)]
    /// #[serde(default, rename_all = "PascalCase")]
    /// struct CompleteMultipartUploadRequestPart {
    ///     #[serde(rename = "PartNumber")]
    ///     part_number: usize,
    ///     #[serde(rename = "ETag", serialize_with = "partial_escape")]
    ///     etag: String,
    /// }
    ///
    /// fn partial_escape<S>(s: &str, ser: S) -> Result<S::Ok, S::Error>
    /// where
    ///     S: serde::Serializer,
    /// {
    ///     ser.serialize_str(&String::from_utf8_lossy(
    ///         &quick_xml::escape::partial_escape(s.as_bytes()),
    ///     ))
    /// }
    /// ```
    ///
    /// ref: <https://github.com/tafia/quick-xml/issues/362>
    #[serde(rename = "ETag")]
    pub etag: String,
}

/// Output of `CompleteMultipartUpload` operation
#[derive(Debug, Default, Deserialize)]
#[serde[default, rename_all = "PascalCase"]]
pub struct CompleteMultipartUploadResult {
    pub location: String,
    pub bucket: String,
    pub key: String,
    #[serde(rename = "ETag")]
    pub etag: String,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct ListObjectsOutput {
    pub name: String,
    pub prefix: String,
    pub contents: Vec<ListObjectsOutputContent>,
    pub common_prefixes: Vec<CommonPrefix>,
    pub marker: String,
    pub next_marker: Option<String>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct CommonPrefix {
    pub prefix: String,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct ListObjectsOutputContent {
    pub key: String,
    pub last_modified: String,
    #[serde(rename = "ETag")]
    pub etag: Option<String>,
    pub size: u64,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OutputCommonPrefix {
    pub prefix: String,
}

/// Output of ListObjectVersions
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct ListObjectVersionsOutput {
    pub is_truncated: Option<bool>,
    pub next_key_marker: Option<String>,
    pub next_version_id_marker: Option<String>,
    pub common_prefixes: Vec<OutputCommonPrefix>,
    pub version: Vec<ListObjectVersionsOutputVersion>,
    pub delete_marker: Vec<ListObjectVersionsOutputDeleteMarker>,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ListObjectVersionsOutputVersion {
    pub key: String,
    pub version_id: String,
    pub is_latest: bool,
    pub size: u64,
    pub last_modified: String,
    #[serde(rename = "ETag")]
    pub etag: Option<String>,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ListObjectVersionsOutputDeleteMarker {
    pub key: String,
    pub version_id: String,
    pub is_latest: bool,
    pub last_modified: String,
}

#[cfg(test)]
mod tests {
    use bytes::Buf;

    use super::*;

    #[test]
    fn test_parse_xml() {
        let bs = bytes::Bytes::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<ListBucketResult>
    <Name>examplebucket</Name>
    <Prefix>obj</Prefix>
    <Marker>obj002</Marker>
    <NextMarker>obj004</NextMarker>
    <MaxKeys>1000</MaxKeys>
    <IsTruncated>false</IsTruncated>
    <Contents>
        <Key>obj002</Key>
        <LastModified>2015-07-01T02:11:19.775Z</LastModified>
        <ETag>"a72e382246ac83e86bd203389849e71d"</ETag>
        <Size>9</Size>
        <Owner>
            <ID>b4bf1b36d9ca43d984fbcb9491b6fce9</ID>
        </Owner>
        <StorageClass>STANDARD</StorageClass>
    </Contents>
    <Contents>
        <Key>obj003</Key>
        <LastModified>2015-07-01T02:11:19.775Z</LastModified>
        <ETag>"a72e382246ac83e86bd203389849e71d"</ETag>
        <Size>10</Size>
        <Owner>
            <ID>b4bf1b36d9ca43d984fbcb9491b6fce9</ID>
        </Owner>
        <StorageClass>STANDARD</StorageClass>
    </Contents>
    <CommonPrefixes>
        <Prefix>hello</Prefix>
    </CommonPrefixes>
    <CommonPrefixes>
        <Prefix>world</Prefix>
    </CommonPrefixes>
</ListBucketResult>"#,
        );
        let out: ListObjectsOutput = quick_xml::de::from_reader(bs.reader()).expect("must success");

        assert_eq!(out.name, "examplebucket".to_string());
        assert_eq!(out.prefix, "obj".to_string());
        assert_eq!(out.marker, "obj002".to_string());
        assert_eq!(out.next_marker, Some("obj004".to_string()),);
        assert_eq!(
            out.contents
                .iter()
                .map(|v| v.key.clone())
                .collect::<Vec<String>>(),
            ["obj002", "obj003"],
        );
        assert_eq!(
            out.contents.iter().map(|v| v.size).collect::<Vec<u64>>(),
            [9, 10],
        );
        assert_eq!(
            out.contents
                .iter()
                .map(|v| v.last_modified.clone())
                .collect::<Vec<String>>(),
            ["2015-07-01T02:11:19.775Z", "2015-07-01T02:11:19.775Z"],
        );
        assert_eq!(
            out.contents
                .iter()
                .map(|v| v.etag.clone())
                .collect::<Vec<Option<String>>>(),
            [
                Some("\"a72e382246ac83e86bd203389849e71d\"".to_string()),
                Some("\"a72e382246ac83e86bd203389849e71d\"".to_string()),
            ],
        );
        assert_eq!(
            out.common_prefixes
                .iter()
                .map(|v| v.prefix.clone())
                .collect::<Vec<String>>(),
            ["hello", "world"],
        )
    }

    #[test]
    fn test_parse_error() {
        let bs = bytes::Bytes::from(
            r#"
<?xml version="1.0" encoding="UTF-8"?>
<Error>
<Code>NoSuchKey</Code>
<Message>The resource you requested does not exist</Message>
<Resource>/example-bucket/object</Resource>
<RequestId>001B21A61C6C0000013402C4616D5285</RequestId>
<HostId>RkRCRDJENDc5MzdGQkQ4OUY3MTI4NTQ3NDk2Mjg0M0FBQUFBQUFBYmJiYmJiYmJD</HostId>
</Error>
"#,
        );

        let out: CosError = de::from_reader(bs.reader()).expect("must success");
        println!("{out:?}");

        assert_eq!(out.code, "NoSuchKey");
        assert_eq!(out.message, "The resource you requested does not exist");
        assert_eq!(out.resource, "/example-bucket/object");
        assert_eq!(out.request_id, "001B21A61C6C0000013402C4616D5285");
        assert_eq!(
            out.host_id,
            "RkRCRDJENDc5MzdGQkQ4OUY3MTI4NTQ3NDk2Mjg0M0FBQUFBQUFBYmJiYmJiYmJD"
        );
    }
}

use bytes::Buf;
use http::StatusCode;
use quick_xml::de;

use opendal_core::Error;
use opendal_core::ErrorKind;

/// CosError is the error returned by cos service.
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct CosError {
    code: String,
    message: String,
    resource: String,
    request_id: String,
    host_id: String,
}

/// Context needed to classify an error from this service.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ErrorContext {
    service_operation: ServiceOperation,
    caller_condition: bool,
    if_not_exists: bool,
}

impl ErrorContext {
    pub(crate) const fn new(service_operation: ServiceOperation) -> Self {
        Self {
            service_operation,
            caller_condition: false,
            if_not_exists: false,
        }
    }

    pub(crate) const fn with_caller_condition(mut self, caller_condition: bool) -> Self {
        self.caller_condition = caller_condition;
        self
    }

    pub(crate) const fn with_if_not_exists(mut self, if_not_exists: bool) -> Self {
        self.caller_condition = self.caller_condition || if_not_exists;
        self.if_not_exists = if_not_exists;
        self
    }
}

/// Parse an error response using its service request context.
pub(crate) fn parse_error(ctx: ErrorContext, resp: Response<Buffer>) -> Error {
    let (parts, body) = resp.into_parts();
    let bs = body.to_bytes();

    let cos_error = de::from_reader::<_, CosError>(bs.clone().reader()).ok();

    let (mut kind, mut retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        StatusCode::PRECONDITION_FAILED | StatusCode::NOT_MODIFIED if ctx.caller_condition => {
            (ErrorKind::ConditionNotMatch, false)
        }
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        // COS could return `520 Origin Error` errors which should be retried.
        v if v.as_u16() == 520 => (ErrorKind::Unexpected, true),

        _ => (ErrorKind::Unexpected, false),
    };

    if let Some(cos_error) = &cos_error {
        match cos_error.code.as_str() {
            "PreconditionFailed" if ctx.caller_condition => {
                (kind, retryable) = (ErrorKind::ConditionNotMatch, false);
            }
            "FileAlreadyExists" if ctx.if_not_exists => {
                (kind, retryable) = (ErrorKind::ConditionNotMatch, false);
            }
            "FileAlreadyExists"
            | "PathConflict"
            | "UploadConflict"
            | "InvalidBucketState"
            | "ObjectLocked"
            | "InvalidObjectState"
            | "RestoreAlreadyInProgress"
            | "ObjectNotAppendable" => {
                (kind, retryable) = (ErrorKind::Conflict, false);
            }
            _ if matches!(
                parts.status,
                StatusCode::CONFLICT | StatusCode::PRECONDITION_FAILED
            ) =>
            {
                (kind, retryable) = (ErrorKind::Unexpected, false);
            }
            _ => {}
        }
    }

    let message = match cos_error {
        Some(cos_error) => format!("{cos_error:?}"),
        None => String::from_utf8_lossy(&bs).into_owned(),
    };

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }
    err
}

#[cfg(test)]
mod error_tests {
    use super::*;

    fn parse_cos_error(ctx: ErrorContext, status: StatusCode, code: &str) -> Error {
        let body = Buffer::from(format!(
            "<Error><Code>{code}</Code><Message>test</Message></Error>"
        ));
        let resp = Response::builder()
            .status(status)
            .body(body)
            .expect("response must build");
        parse_error(ctx, resp)
    }

    #[test]
    fn conflict_classification_uses_native_code_and_condition() {
        let conditional = ErrorContext::new(ServiceOperation("PutObject")).with_if_not_exists(true);
        assert_eq!(
            parse_cos_error(conditional, StatusCode::CONFLICT, "FileAlreadyExists").kind(),
            ErrorKind::ConditionNotMatch
        );
        assert_eq!(
            parse_cos_error(
                conditional,
                StatusCode::PRECONDITION_FAILED,
                "PreconditionFailed"
            )
            .kind(),
            ErrorKind::ConditionNotMatch
        );

        let unconditional = ErrorContext::new(ServiceOperation("PutObject"));
        for code in [
            "PathConflict",
            "UploadConflict",
            "ObjectLocked",
            "InvalidObjectState",
        ] {
            assert_eq!(
                parse_cos_error(unconditional, StatusCode::CONFLICT, code).kind(),
                ErrorKind::Conflict,
                "native code {code}"
            );
        }
        assert_eq!(
            parse_cos_error(unconditional, StatusCode::CONFLICT, "UnknownConflict").kind(),
            ErrorKind::Unexpected
        );
    }
}
