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

use std::sync::Arc;
use std::sync::OnceLock;

use bytes::Buf;
use http::Response;
use http::StatusCode;

use crate::core::parse_error;
use crate::core::*;
use opendal_core::raw::*;
use opendal_core::*;

pub type S3Copiers = oio::MultipartCopier<S3Copier>;

fn is_immutable_source_version(version: &str) -> bool {
    !version.is_empty() && version != "null"
}

pub fn new_s3_copier(
    core: Arc<S3Core>,
    ctx: &OperationContext,
    from: &str,
    to: &str,
    args: OpCopy,
) -> Result<S3Copiers> {
    let capability = core.capability;
    let max_part_size = capability.copy_multi_max_size.ok_or_else(|| {
        Error::new(
            ErrorKind::Unexpected,
            "multipart copy requires copy_multi_max_size capability",
        )
    })?;

    let (copy_once_threshold, part_size) = match args.chunk() {
        Some(chunk) => {
            let min_part_size = capability.copy_multi_min_size.ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "multipart copy requires copy_multi_min_size capability",
                )
            })?;
            let part_size = chunk.clamp(min_part_size, max_part_size) as u64;
            (part_size.saturating_sub(1), part_size)
        }
        None => {
            let part_size = max_part_size as u64;
            (part_size, part_size)
        }
    };
    let source_content_length_hint = args.source_content_length_hint();
    let concurrent = args.concurrent();

    Ok(oio::MultipartCopier::new(
        (ctx.executor().clone(), capability),
        S3Copier {
            core,
            ctx: ctx.clone(),
            from: from.to_string(),
            to: to.to_string(),
            args,
            source_snapshot: OnceLock::new(),
        },
        source_content_length_hint,
        copy_once_threshold,
        part_size,
        concurrent,
    ))
}

pub struct S3Copier {
    core: Arc<S3Core>,
    ctx: OperationContext,
    from: String,
    to: String,
    args: OpCopy,
    // An explicit source length hint intentionally skips loading this snapshot.
    source_snapshot: OnceLock<Metadata>,
}

struct S3CopySource<'a> {
    version: Option<&'a str>,
    if_match: Option<&'a str>,
}

impl S3Copier {
    fn error_context(
        &self,
        service_operation: ServiceOperation,
        source_if_match: bool,
    ) -> ErrorContext {
        ErrorContext::new(service_operation)
            .with_caller_condition(self.args.is_conditional())
            .with_source_if_match(source_if_match)
    }

    fn copy_source(&self) -> Result<S3CopySource<'_>> {
        let snapshot = self.source_snapshot.get();
        let version = self.args.source_version().or_else(|| {
            snapshot
                .and_then(Metadata::version)
                .filter(|version| is_immutable_source_version(version))
        });
        let if_match = snapshot.and_then(Metadata::etag);

        if version.is_none()
            && if_match.is_none()
            && self.args.source_content_length_hint().is_none()
        {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "S3 copy source does not have an immutable version or ETag",
            )
            .with_operation("S3Copier::copy_source"));
        }

        Ok(S3CopySource { version, if_match })
    }
}

impl oio::MultipartCopy for S3Copier {
    async fn source_metadata(&self) -> Result<Metadata> {
        if let Some(metadata) = self.source_snapshot.get() {
            return Ok(metadata.clone());
        }

        let args = options::StatOptions {
            version: self.args.source_version().map(str::to_owned),
            ..Default::default()
        }
        .into();

        let resp = self
            .core
            .s3_head_object(&self.ctx, &self.from, args)
            .await?;

        match resp.status() {
            StatusCode::OK => {
                let headers = resp.headers();
                let mut metadata = parse_into_metadata(&self.from, headers)?.into_builder();
                if let Some(version) = parse_header_to_str(headers, constants::X_AMZ_VERSION_ID)?
                    .or(self.args.source_version())
                {
                    metadata.version(version);
                }
                let metadata = metadata.build();
                let _ = self.source_snapshot.set(metadata.clone());
                Ok(self.source_snapshot.get().cloned().unwrap_or(metadata))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("HeadObject")),
                resp,
            )),
        }
    }

    async fn copy_once(&self) -> Result<Metadata> {
        let source = self.copy_source()?;
        let resp = self
            .core
            .s3_copy_object(
                &self.ctx,
                &self.from,
                &self.to,
                source.version,
                source.if_match,
                &self.args,
            )
            .await?;

        match resp.status() {
            StatusCode::OK => {
                let (parts, body) = resp.into_parts();
                let bs = body.to_bytes();
                let version = parse_header_to_str(&parts.headers, constants::X_AMZ_VERSION_ID)?
                    .map(str::to_string);

                let result: CopyObjectResult =
                    quick_xml::de::from_reader(bs.as_ref()).map_err(new_xml_deserialize_error)?;

                // S3 may return 200 OK with an <Error> body for CopyObject.
                if result.etag.is_empty() {
                    let err = parse_error(
                        self.error_context(
                            ServiceOperation("CopyObject"),
                            source.if_match.is_some(),
                        ),
                        Response::from_parts(parts, Buffer::from(bs)),
                    );
                    return if self.args.if_match().is_some() && err.kind() == ErrorKind::NotFound {
                        Err(Error::new(
                            ErrorKind::ConditionNotMatch,
                            "copy precondition requires a live destination",
                        ))
                    } else {
                        Err(err)
                    };
                }

                let mut meta = if self.to.ends_with('/') {
                    MetadataBuilder::dir()
                } else {
                    MetadataBuilder::unknown()
                };
                meta.etag(&result.etag);
                if !result.last_modified.is_empty() {
                    meta.last_modified(result.last_modified.parse()?);
                }
                if let Some(version) = version {
                    meta.version(&version);
                }

                Ok(meta.build())
            }
            _ => {
                let err = parse_error(
                    self.error_context(ServiceOperation("CopyObject"), source.if_match.is_some()),
                    resp,
                );
                if self.args.if_match().is_some() && err.kind() == ErrorKind::NotFound {
                    Err(Error::new(
                        ErrorKind::ConditionNotMatch,
                        "copy precondition requires a live destination",
                    ))
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn initiate_copy(&self) -> Result<String> {
        let resp = self
            .core
            .s3_initiate_multipart_copy(&self.ctx, &self.to)
            .await?;

        match resp.status() {
            StatusCode::OK => {
                let bs = resp.into_body();

                let result: InitiateMultipartUploadResult =
                    quick_xml::de::from_reader(bs.reader()).map_err(new_xml_deserialize_error)?;

                Ok(result.upload_id)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateMultipartUpload")),
                resp,
            )),
        }
    }

    async fn copy_part(
        &self,
        upload_id: &str,
        part_number: usize,
        range: BytesRange,
    ) -> Result<oio::MultipartPart> {
        let size = range.size().expect("multipart copy range must be sized");
        let part_number = part_number + 1;
        let source = self.copy_source()?;

        let req = self
            .core
            .s3_upload_part_copy_request(S3UploadPartCopyRequest {
                from: &self.from,
                to: &self.to,
                source_version: source.version,
                if_match: source.if_match,
                if_none_match: None,
                if_modified_since: None,
                if_unmodified_since: None,
                upload_id,
                part_number,
                range,
                operation: Operation::Copy,
            })?;

        let resp = self
            .core
            .send(&self.ctx, req, self.core.signers.iam())
            .await?;

        match resp.status() {
            StatusCode::OK => {
                let (parts, body) = resp.into_parts();
                let bs = body.to_bytes();
                let result: CopyObjectResult =
                    quick_xml::de::from_reader(bs.as_ref()).map_err(new_xml_deserialize_error)?;

                // S3 may return 200 OK with an <Error> body for UploadPartCopy.
                if result.etag.is_empty() {
                    return Err(parse_error(
                        self.error_context(
                            ServiceOperation("UploadPartCopy"),
                            source.if_match.is_some(),
                        ),
                        Response::from_parts(parts, Buffer::from(bs)),
                    ));
                }

                Ok(oio::MultipartPart {
                    part_number,
                    etag: result.etag,
                    checksum: None,
                    size: Some(size),
                })
            }
            _ => Err(parse_error(
                self.error_context(
                    ServiceOperation("UploadPartCopy"),
                    source.if_match.is_some(),
                ),
                resp,
            )),
        }
    }

    async fn complete_copy(
        &self,
        upload_id: &str,
        parts: &[oio::MultipartPart],
    ) -> Result<Metadata> {
        let parts = parts
            .iter()
            .map(|p| CompleteMultipartUploadRequestPart {
                part_number: p.part_number,
                etag: p.etag.clone(),
                ..Default::default()
            })
            .collect();

        let resp = self
            .core
            .s3_complete_multipart_copy(&self.ctx, &self.to, upload_id, parts, &self.args)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let (parts, body) = resp.into_parts();
                let bs = body.to_bytes();
                let version = parse_header_to_str(&parts.headers, constants::X_AMZ_VERSION_ID)?
                    .map(str::to_string);

                let ret: CompleteMultipartUploadResult =
                    quick_xml::de::from_reader(bs.as_ref()).map_err(new_xml_deserialize_error)?;
                // S3 may return 200 OK with an <Error> body for CompleteMultipartUpload.
                if ret.etag.is_empty() {
                    let err = parse_error(
                        self.error_context(ServiceOperation("CompleteMultipartUpload"), false),
                        Response::from_parts(parts, Buffer::from(bs)),
                    );
                    return if self.args.if_match().is_some() && err.kind() == ErrorKind::NotFound {
                        Err(Error::new(
                            ErrorKind::ConditionNotMatch,
                            "copy precondition requires a live destination",
                        ))
                    } else {
                        Err(err)
                    };
                }

                let mut meta = if self.to.ends_with('/') {
                    MetadataBuilder::dir()
                } else {
                    MetadataBuilder::unknown()
                };
                meta.etag(&ret.etag);
                if let Some(version) = version {
                    meta.version(&version);
                }

                Ok(meta.build())
            }
            _ => {
                let err = parse_error(
                    self.error_context(ServiceOperation("CompleteMultipartUpload"), false),
                    resp,
                );
                if self.args.if_match().is_some() && err.kind() == ErrorKind::NotFound {
                    Err(Error::new(
                        ErrorKind::ConditionNotMatch,
                        "copy precondition requires a live destination",
                    ))
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn abort_copy(&self, upload_id: &str) -> Result<()> {
        let resp = self
            .core
            .s3_abort_multipart_copy(&self.ctx, &self.to, upload_id)
            .await?;
        match resp.status() {
            StatusCode::NO_CONTENT => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("AbortMultipartUpload")),
                resp,
            )),
        }
    }
}
