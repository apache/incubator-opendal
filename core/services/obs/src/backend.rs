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
use std::sync::Arc;

use http::StatusCode;
use http::Uri;
use log::debug;
use opendal_core::raw::*;
use opendal_core::*;
use reqsign_core::Context;
use reqsign_core::OsEnv;
use reqsign_core::ProvideCredentialChain;
use reqsign_core::Signer;
use reqsign_file_read_tokio::TokioFileRead;
use reqsign_huaweicloud_obs::EnvCredentialProvider;
use reqsign_huaweicloud_obs::RequestSigner;
use reqsign_huaweicloud_obs::StaticCredentialProvider;

use super::OBS_SCHEME;
use super::config::ObsConfig;
use super::core::constants;
use super::core::parse_error;
use super::core::{ErrorContext, ObsCore};
use super::deleter::ObsDeleter;
use super::lister::ObsLister;
use super::reader::*;
use super::writer::ObsWriter;
use super::writer::ObsWriters;

/// Huawei-Cloud Object Storage Service (OBS) support
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct ObsBuilder {
    pub(super) config: ObsConfig,
}

impl Debug for ObsBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObsBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl ObsBuilder {
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

    /// Set endpoint of this backend.
    ///
    /// Both huaweicloud default domain and user domain endpoints are allowed.
    /// Please DO NOT add the bucket name to the endpoint.
    ///
    /// - `https://obs.cn-north-4.myhuaweicloud.com`
    /// - `obs.cn-north-4.myhuaweicloud.com` (https by default)
    /// - `https://custom.obs.com` (port should not be set)
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            self.config.endpoint = Some(endpoint.trim_end_matches('/').to_string());
        }

        self
    }

    /// Set access_key_id of this backend.
    /// - If it is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn access_key_id(mut self, access_key_id: &str) -> Self {
        if !access_key_id.is_empty() {
            self.config.access_key_id = Some(access_key_id.to_string());
        }

        self
    }

    /// Set secret_access_key of this backend.
    /// - If it is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn secret_access_key(mut self, secret_access_key: &str) -> Self {
        if !secret_access_key.is_empty() {
            self.config.secret_access_key = Some(secret_access_key.to_string());
        }

        self
    }

    /// Set bucket of this backend.
    /// The param is required.
    pub fn bucket(mut self, bucket: &str) -> Self {
        if !bucket.is_empty() {
            self.config.bucket = Some(bucket.to_string());
        }

        self
    }

    /// Deprecated: OBS versioning capability is not controlled by service config.
    #[deprecated(
        since = "0.57.0",
        note = "OBS versioning capability is not controlled by this option and this option is no longer needed."
    )]
    pub fn enable_versioning(self, _enabled: bool) -> Self {
        self
    }
}

impl Builder for ObsBuilder {
    type Config = ObsConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let bucket = match &self.config.bucket {
            Some(bucket) => Ok(bucket.to_string()),
            None => Err(
                Error::new(ErrorKind::ConfigInvalid, "The bucket is misconfigured")
                    .with_context("service", OBS_SCHEME),
            ),
        }?;
        debug!("backend use bucket {}", bucket);

        let uri = match &self.config.endpoint {
            Some(endpoint) => endpoint.parse::<Uri>().map_err(|err| {
                Error::new(ErrorKind::ConfigInvalid, "endpoint is invalid")
                    .with_context("service", OBS_SCHEME)
                    .set_source(err)
            }),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_context("service", OBS_SCHEME)),
        }?;

        let scheme = match uri.scheme_str() {
            Some(scheme) => scheme.to_string(),
            None => "https".to_string(),
        };

        let (endpoint, is_obs_default) = {
            let host = uri.host().unwrap_or_default().to_string();
            if host.starts_with("obs.")
                && (host.ends_with(".myhuaweicloud.com") || host.ends_with(".huawei.com"))
            {
                (format!("{bucket}.{host}"), true)
            } else {
                (host, false)
            }
        };
        debug!("backend use endpoint {}", endpoint);

        let ctx = Context::new().with_file_read(TokioFileRead).with_env(OsEnv);

        let mut provider = ProvideCredentialChain::new().push(EnvCredentialProvider::new());

        if let (Some(ak), Some(sk)) = (&self.config.access_key_id, &self.config.secret_access_key) {
            let static_provider = StaticCredentialProvider::new(ak, sk);
            provider = provider.push_front(static_provider);
        }

        // Set the bucket name in CanonicalizedResource.
        // 1. If the bucket is bound to a user domain name, use the user domain name as the bucket name,
        // for example, `/obs.ccc.com/object`. `obs.ccc.com` is the user domain name bound to the bucket.
        // 2. If you do not access OBS using a user domain name, this field is in the format of `/bucket/object`.
        //
        // Please refer to this doc for more details:
        // https://support.huaweicloud.com/intl/en-us/api-obs/obs_04_0010.html
        let request_signer = RequestSigner::new(if is_obs_default { &bucket } else { &endpoint });
        let signer = Signer::new(ctx, provider, request_signer);

        let info = ServiceInfo::new(OBS_SCHEME, &root, &bucket);
        let capability = Capability {
            stat: true,
            stat_with_if_match: true,
            stat_with_if_none_match: true,

            read: true,
            read_with_suffix: true,

            read_with_if_match: true,
            read_with_if_none_match: true,
            read_with_if_modified_since: true,
            read_with_if_unmodified_since: true,

            write: true,
            write_can_empty: true,
            write_can_append: true,
            write_can_multi: true,
            write_with_content_type: true,
            write_with_cache_control: true,
            // The min multipart size of OBS is 5 MiB.
            //
            // ref: <https://support.huaweicloud.com/intl/en-us/ugobs-obs/obs_41_0021.html>
            write_multi_min_size: Some(5 * 1024 * 1024),
            // The max multipart size of OBS is 5 GiB.
            //
            // ref: <https://support.huaweicloud.com/intl/en-us/ugobs-obs/obs_41_0021.html>
            write_multi_max_size: if cfg!(target_pointer_width = "64") {
                Some(5 * 1024 * 1024 * 1024)
            } else {
                Some(usize::MAX)
            },
            write_with_user_metadata: true,

            delete: true,
            copy: true,

            list: true,
            list_with_recursive: true,

            presign: true,
            presign_stat: true,
            presign_read: true,
            presign_write: true,

            shared: true,

            ..Default::default()
        };

        debug!("backend build finished");
        Ok(ObsBackend {
            core: Arc::new(ObsCore {
                info,
                capability,
                bucket,
                root,
                endpoint: format!("{}://{}", scheme, endpoint),
                signer,
            }),
        })
    }
}

/// Backend for Huaweicloud OBS services.
#[derive(Debug, Clone)]
pub struct ObsBackend {
    pub(crate) core: Arc<ObsCore>,
}

impl Service for ObsBackend {
    type Reader = oio::StreamReader<ObsReader>;
    type Writer = ObsWriters;
    type Lister = oio::PageLister<ObsLister>;
    type Deleter = oio::OneShotDeleter<ObsDeleter>;
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
        let resp = self.core.obs_head_object(ctx, path, &args).await?;
        let headers = resp.headers();

        let status = resp.status();

        // The response is very similar to azblob.
        match status {
            StatusCode::OK => {
                let meta = parse_into_metadata(path, headers)?;
                let user_meta = headers
                    .iter()
                    .filter_map(|(name, _)| {
                        name.as_str()
                            .strip_prefix(constants::X_OBS_META_PREFIX)
                            .and_then(|stripped_key| {
                                parse_header_to_str(headers, name)
                                    .unwrap_or(None)
                                    .map(|val| (stripped_key.to_string(), val.to_string()))
                            })
                    })
                    .collect::<HashMap<_, _>>();

                let mut meta = meta.into_builder();
                if !user_meta.is_empty() {
                    meta.user_metadata(user_meta);
                }

                if let Some(v) = parse_header_to_str(headers, constants::X_OBS_VERSION_ID)? {
                    meta.version(v);
                }

                Ok(RpStat::new(meta.build()))
            }
            StatusCode::NOT_FOUND if path.ends_with('/') => {
                Ok(RpStat::new(MetadataBuilder::dir().build()))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("HeadObject")),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<ObsReader> = {
            Ok(oio::StreamReader::new(ObsReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: ObsWriters = {
            let writer = ObsWriter::new(self.core.clone(), ctx.clone(), path, args.clone());

            let w = if args.append() {
                ObsWriters::Two(oio::AppendWriter::new(writer))
            } else {
                ObsWriters::One(oio::MultipartWriter::new(
                    ctx.executor().clone(),
                    writer,
                    args.concurrent(),
                ))
            };

            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<ObsDeleter> = {
            Ok(oio::OneShotDeleter::new(ObsDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<ObsLister> = {
            let l = ObsLister::new(
                self.core.clone(),
                ctx.clone(),
                path,
                args.recursive(),
                args.limit(),
            );
            Ok(oio::PageLister::new(l))
        }?;

        Ok(output)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        let backend = self.clone();
        let core = self.core.clone();
        let ctx = ctx.clone();
        let from = from.to_string();
        let to = to.to_string();
        let source_content_length_hint = args.source_content_length_hint();
        Ok(oio::OneShotCopier::new(async move {
            let source_size = match source_content_length_hint {
                Some(size) => size,
                None => backend
                    .stat(&ctx, &from, OpStat::default())
                    .await?
                    .into_metadata()
                    .content_length(),
            };

            let resp = core.obs_copy_object(&ctx, &from, &to).await?;

            let status = resp.status();

            match status {
                StatusCode::OK => Ok(MetadataBuilder::file(source_size).build()),
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

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        let req = match args.operation() {
            PresignOperation::Stat(v) => self.core.obs_head_object_request(path, v),
            PresignOperation::Read(range, v) => self.core.obs_get_object_request(path, *range, v),
            PresignOperation::Write(v) => {
                self.core
                    .obs_put_object_request(path, None, v, Buffer::new())
            }
            PresignOperation::Delete(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            )),
            _ => Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            )),
        };
        let req = req?;
        let req = self.core.sign_query(ctx, req, args.expire()).await?;

        // We don't need this request anymore, consume it directly.
        let (parts, _) = req.into_parts();

        Ok(RpPresign::new(PresignedRequest::new(
            parts.method,
            parts.uri,
            parts.headers,
        )))
    }
}
