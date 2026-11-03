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
use std::sync::Arc;

use http::StatusCode;
use log::debug;

use super::SWIFT_SCHEME;
use super::SwiftConfig;
use super::core::parse_error;
use super::core::*;
use super::deleter::SwiftDeleter;
use super::lister::SwiftLister;
use super::reader::*;
use super::writer::SwiftWriter;
use opendal_core::raw::*;
use opendal_core::*;

/// [OpenStack Swift](https://docs.openstack.org/api-ref/object-store/#)'s REST API support.
/// For more information about swift-compatible services, refer to [Compatible Services](#compatible-services).
#[doc = include_str!("docs.md")]
#[doc = include_str!("compatible_services.md")]
#[derive(Debug, Default)]
pub struct SwiftBuilder {
    pub(super) config: SwiftConfig,
}

impl SwiftBuilder {
    /// Set the remote address of this backend
    ///
    /// Endpoints should be full uri, e.g.
    ///
    /// - `http://127.0.0.1:8080/v1/AUTH_test`
    /// - `http://192.168.66.88:8080/swift/v1`
    /// - `https://openstack-controller.example.com:8080/v1/ccount`
    ///
    /// If user inputs endpoint without scheme, we will
    /// prepend `https://` to it.
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.config.endpoint = if endpoint.is_empty() {
            None
        } else {
            Some(endpoint.trim_end_matches('/').to_string())
        };
        self
    }

    /// Set container of this backend.
    ///
    /// All operations will happen under this container. It is required. e.g. `snapshots`
    pub fn container(mut self, container: &str) -> Self {
        self.config.container = if container.is_empty() {
            None
        } else {
            Some(container.trim_end_matches('/').to_string())
        };
        self
    }

    /// Set root of this backend.
    ///
    /// All operations will happen under this root.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Set the token of this backend.
    ///
    /// Default to empty string.
    pub fn token(mut self, token: &str) -> Self {
        if !token.is_empty() {
            self.config.token = Some(token.to_string());
        }
        self
    }

    /// Set the TempURL key for generating presigned URLs.
    ///
    /// This should match the `X-Account-Meta-Temp-URL-Key` or
    /// `X-Container-Meta-Temp-URL-Key` value configured on the Swift
    /// account or container.
    pub fn temp_url_key(mut self, key: &str) -> Self {
        if !key.is_empty() {
            self.config.temp_url_key = Some(key.to_string());
        }
        self
    }

    /// Set the hash algorithm for TempURL signing.
    ///
    /// Supported values: `sha1`, `sha256`, `sha512`. Defaults to `sha256`.
    /// The cluster must have the chosen algorithm in its
    /// `tempurl.allowed_digests` (check `GET /info`).
    pub fn temp_url_hash_algorithm(mut self, algo: &str) -> Self {
        if !algo.is_empty() {
            self.config.temp_url_hash_algorithm = Some(algo.to_string());
        }
        self
    }
}

impl Builder for SwiftBuilder {
    type Config = SwiftConfig;

    /// Build a SwiftBackend.
    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let endpoint = match self.config.endpoint {
            Some(endpoint) => {
                if endpoint.starts_with("http") {
                    endpoint
                } else {
                    format!("https://{endpoint}")
                }
            }
            None => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "missing endpoint for Swift",
                ));
            }
        };
        debug!("backend use endpoint: {}", endpoint);

        let container = match self.config.container {
            Some(container) => container,
            None => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "missing container for Swift",
                ));
            }
        };

        let token = self.config.token.unwrap_or_default();
        let temp_url_key = self.config.temp_url_key.unwrap_or_default();
        let has_temp_url_key = !temp_url_key.is_empty();
        let temp_url_hash_algorithm = match &self.config.temp_url_hash_algorithm {
            Some(algo) => TempUrlHashAlgorithm::from_str_opt(algo)?,
            None => TempUrlHashAlgorithm::Sha256,
        };

        Ok(SwiftBackend {
            core: Arc::new(SwiftCore {
                info: ServiceInfo::new(SWIFT_SCHEME, &root, ""),
                capability: Capability {
                    stat: true,
                    stat_with_if_match: true,
                    stat_with_if_none_match: true,
                    stat_with_if_modified_since: true,
                    stat_with_if_unmodified_since: true,

                    read: true,
                    read_with_suffix: true,
                    read_with_if_match: true,
                    read_with_if_none_match: true,
                    read_with_if_modified_since: true,
                    read_with_if_unmodified_since: true,

                    write: true,
                    write_can_empty: true,
                    write_can_multi: true,
                    write_multi_min_size: Some(5 * 1024 * 1024),
                    write_multi_max_size: if cfg!(target_pointer_width = "64") {
                        Some(5 * 1024 * 1024 * 1024)
                    } else {
                        Some(usize::MAX)
                    },
                    write_with_content_type: true,
                    write_with_content_disposition: true,
                    write_with_content_encoding: true,
                    write_with_cache_control: true,
                    write_with_user_metadata: true,

                    delete: true,
                    delete_max_size: Some(10000),

                    copy: true,

                    list: true,
                    list_with_recursive: true,
                    list_with_start_after: true,

                    presign: has_temp_url_key,
                    presign_stat: has_temp_url_key,
                    presign_read: has_temp_url_key,
                    presign_write: has_temp_url_key,

                    shared: true,

                    ..Default::default()
                },
                root,
                endpoint,
                container,
                token,
                temp_url_key,
                temp_url_hash_algorithm,
            }),
        })
    }
}

/// Backend for Swift service
#[derive(Debug, Clone)]
pub struct SwiftBackend {
    pub(crate) core: Arc<SwiftCore>,
}

impl Service for SwiftBackend {
    type Reader = oio::StreamReader<SwiftReader>;
    type Writer = oio::MultipartWriter<SwiftWriter>;
    type Lister = oio::PageLister<SwiftLister>;
    type Deleter = oio::BatchDeleter<SwiftDeleter>;
    type Copier = oio::OneShotCopier;
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.core.info.clone()
    }

    fn capability(&self) -> Capability {
        self.core.capability
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let resp = self.core.swift_get_metadata(ctx, path, &args).await?;

        match resp.status() {
            StatusCode::OK | StatusCode::NO_CONTENT => {
                let headers = resp.headers();
                let mut meta = parse_into_metadata(path, headers)?.into_builder();
                let user_meta = parse_prefixed_headers(headers, "x-object-meta-");
                if !user_meta.is_empty() {
                    meta.user_metadata(user_meta);
                }

                Ok(RpStat::new(meta.build()))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("ShowObjectMetadata")),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<SwiftReader> = {
            Ok(oio::StreamReader::new(SwiftReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: oio::MultipartWriter<SwiftWriter> = {
            let concurrent = args.concurrent();
            let writer = SwiftWriter::new(
                self.core.clone(),
                ctx.clone(),
                args.clone(),
                path.to_string(),
            );
            let w = oio::MultipartWriter::new(ctx.executor().clone(), writer, concurrent);

            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::BatchDeleter<SwiftDeleter> = {
            Ok(oio::BatchDeleter::new(
                SwiftDeleter::new(self.core.clone(), ctx.clone()),
                self.core.capability.delete_max_size,
            ))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<SwiftLister> = {
            let l = SwiftLister::new(
                self.core.clone(),
                ctx.clone(),
                path.to_string(),
                args.recursive(),
                args.limit(),
                args.start_after().map(String::from),
            );

            Ok(oio::PageLister::new(l))
        }?;

        Ok(output)
    }

    async fn presign(
        &self,
        _ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        let (expire, op) = args.into_parts();

        let method = match &op {
            PresignOperation::Stat(_) => http::Method::HEAD,
            PresignOperation::Read(_, _) => http::Method::GET,
            PresignOperation::Write(_) => http::Method::PUT,
            _ => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "presign operation is not supported",
                ));
            }
        };

        let url = self.core.swift_temp_url(&method, path, expire)?;
        let uri: http::Uri = url.parse().map_err(|e| {
            Error::new(ErrorKind::Unexpected, "failed to parse presigned URL").set_source(e)
        })?;

        Ok(RpPresign::new(PresignedRequest::new(
            method,
            uri,
            http::HeaderMap::new(),
        )))
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        let core = self.core.clone();
        let ctx = ctx.clone();
        let from = from.to_string();
        let to = to.to_string();

        Ok(oio::OneShotCopier::new(async move {
            let source_size = match args.source_content_length_hint() {
                Some(size) => size,
                None => {
                    let stat_args: OpStat = options::StatOptions {
                        version: args.source_version().map(str::to_owned),
                        ..Default::default()
                    }
                    .into();
                    let resp = core.swift_get_metadata(&ctx, &from, &stat_args).await?;
                    match resp.status() {
                        StatusCode::OK | StatusCode::NO_CONTENT => {
                            parse_into_metadata(&from, resp.headers())?.content_length()
                        }
                        _ => {
                            return Err(parse_error(
                                ErrorContext::new(ServiceOperation("ShowObjectMetadata")),
                                resp,
                            ));
                        }
                    }
                }
            };
            // cannot copy objects larger than 5 GB.
            // Reference: https://docs.openstack.org/api-ref/object-store/#copy-object
            let resp = core.swift_copy(&ctx, &from, &to).await?;

            let status = resp.status();

            match status {
                StatusCode::CREATED | StatusCode::OK => {
                    Ok(MetadataBuilder::file(source_size).build())
                }
                _ => Err(parse_error(
                    ErrorContext::new(ServiceOperation("CopyObject")),
                    resp,
                )),
            }
        }))
    }

    async fn rename(
        &self,
        _ctx: &OperationContext,
        _from: &str,
        _to: &str,
        _args: OpRename,
    ) -> Result<RpRename> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }
}
