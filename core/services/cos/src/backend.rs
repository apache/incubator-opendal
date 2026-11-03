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

use bytes::Buf;
use http::StatusCode;
use http::Uri;
use log::debug;
use reqsign_core::Context;
use reqsign_core::OsEnv;
use reqsign_core::Signer;
use reqsign_file_read_tokio::TokioFileRead;
use reqsign_tencent_cos::DefaultCredentialProvider;
use reqsign_tencent_cos::RequestSigner;
use reqsign_tencent_cos::StaticCredentialProvider;

use super::COS_SCHEME;
use super::config::CosConfig;
use super::core::parse_error;
use super::core::*;
use super::deleter::CosDeleter;
use super::lister::CosLister;
use super::lister::CosListers;
use super::lister::CosObjectVersionsLister;
use super::reader::*;
use super::writer::CosWriter;
use super::writer::CosWriters;
use opendal_core::raw::*;
use opendal_core::*;

/// Tencent-Cloud COS services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct CosBuilder {
    pub(super) config: CosConfig,
}

impl Debug for CosBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CosBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl CosBuilder {
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
    /// NOTE: no bucket or account id in endpoint, we will trim them if exists.
    ///
    /// # Examples
    ///
    /// - `https://cos.ap-singapore.myqcloud.com`
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            self.config.endpoint = Some(endpoint.trim_end_matches('/').to_string());
        }

        self
    }

    /// Set secret_id of this backend.
    /// - If it is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn secret_id(mut self, secret_id: &str) -> Self {
        if !secret_id.is_empty() {
            self.config.secret_id = Some(secret_id.to_string());
        }

        self
    }

    /// Set secret_key of this backend.
    /// - If it is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn secret_key(mut self, secret_key: &str) -> Self {
        if !secret_key.is_empty() {
            self.config.secret_key = Some(secret_key.to_string());
        }

        self
    }

    /// Set security_token (a.k.a. session token) of this backend.
    ///
    /// This is used when authenticating via Tencent Cloud STS temporary
    /// credentials (e.g. obtained from `GetFederationToken` or
    /// `AssumeRole`). When provided, it will be combined with `secret_id`
    /// and `secret_key` to sign requests, and the `x-cos-security-token`
    /// header will be attached automatically.
    ///
    /// - If this is set along with `secret_id` and `secret_key`, a static
    ///   credential provider with the token will be used.
    /// - If this is not set, the default credential chain in reqsign will
    ///   try to load credentials (including the token) from environment
    ///   variables such as `TENCENTCLOUD_TOKEN`,
    ///   `TENCENTCLOUD_SECURITY_TOKEN`, and `QCLOUD_SECRET_TOKEN`
    ///   (unless `disable_config_load` is enabled).
    pub fn security_token(mut self, security_token: &str) -> Self {
        if !security_token.is_empty() {
            self.config.security_token = Some(security_token.to_string());
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

    /// Deprecated: COS versioning capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "COS versioning capability is enabled by default and this option is no longer needed."
    )]
    pub fn enable_versioning(self, _enabled: bool) -> Self {
        self
    }

    /// Disable config load so that opendal will not load config from
    /// environment.
    ///
    /// For examples:
    ///
    /// - envs like `TENCENTCLOUD_SECRET_ID`
    pub fn disable_config_load(mut self) -> Self {
        self.config.disable_config_load = true;
        self
    }
}

impl Builder for CosBuilder {
    type Config = CosConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let bucket = match &self.config.bucket {
            Some(bucket) => Ok(bucket.to_string()),
            None => Err(
                Error::new(ErrorKind::ConfigInvalid, "The bucket is misconfigured")
                    .with_context("service", COS_SCHEME),
            ),
        }?;
        debug!("backend use bucket {}", bucket);

        let uri = match &self.config.endpoint {
            Some(endpoint) => endpoint.parse::<Uri>().map_err(|err| {
                Error::new(ErrorKind::ConfigInvalid, "endpoint is invalid")
                    .with_context("service", COS_SCHEME)
                    .with_context("endpoint", endpoint)
                    .set_source(err)
            }),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_context("service", COS_SCHEME)),
        }?;

        let endpoint = build_endpoint(&uri, &bucket)?;
        debug!("backend use endpoint {}", endpoint);

        let os_env = OsEnv;
        let ctx = Context::new()
            .with_file_read(TokioFileRead)
            .with_env(os_env);

        let mut credential = if self.config.disable_config_load {
            DefaultCredentialProvider::builder()
                .no_env()
                .no_web_identity()
                .build()
        } else {
            DefaultCredentialProvider::new()
        };

        if let (Some(secret_id), Some(secret_key)) = (
            self.config.secret_id.as_deref(),
            self.config.secret_key.as_deref(),
        ) {
            let static_provider = if let Some(token) = self.config.security_token.as_deref() {
                StaticCredentialProvider::with_security_token(secret_id, secret_key, token)
            } else {
                StaticCredentialProvider::new(secret_id, secret_key)
            };

            credential = credential.push_front(static_provider);
        }

        let signer = Signer::new(ctx, credential, RequestSigner::new());

        let info = ServiceInfo::new(COS_SCHEME, &root, &bucket);
        let capability = Capability {
            stat: true,
            stat_with_if_match: true,
            stat_with_if_none_match: true,
            stat_with_version: true,

            read: true,
            read_with_suffix: true,

            read_with_if_match: true,
            read_with_if_none_match: true,
            read_with_if_modified_since: true,
            read_with_if_unmodified_since: true,
            read_with_version: true,

            write: true,
            write_can_empty: true,
            write_can_append: true,
            write_can_multi: true,
            write_with_content_type: true,
            write_with_cache_control: true,
            write_with_content_disposition: true,
            write_with_if_not_exists: true,
            copy_with_if_not_exists: true,
            copy_with_source_version: true,
            restore: true,
            restore_with_version: true,
            // The min multipart size of COS is 1 MiB.
            //
            // ref: <https://www.tencentcloud.com/document/product/436/14112>
            write_multi_min_size: Some(1024 * 1024),
            // The max multipart size of COS is 5 GiB.
            //
            // ref: <https://www.tencentcloud.com/document/product/436/14112>
            write_multi_max_size: if cfg!(target_pointer_width = "64") {
                Some(5 * 1024 * 1024 * 1024)
            } else {
                Some(usize::MAX)
            },
            write_with_user_metadata: true,

            delete: true,
            delete_with_version: true,
            copy: true,

            list: true,
            list_with_recursive: true,
            list_with_versions: true,
            list_with_deleted: true,

            presign: true,
            presign_stat: true,
            presign_read: true,
            presign_write: true,

            shared: true,

            ..Default::default()
        };

        Ok(CosBackend {
            core: Arc::new(CosCore {
                info,
                capability,
                bucket: bucket.clone(),
                root,
                endpoint,
                signer,
            }),
        })
    }
}

/// Backend for Tencent-Cloud COS services.
#[derive(Debug, Clone)]
pub struct CosBackend {
    pub(crate) core: Arc<CosCore>,
}

impl Service for CosBackend {
    type Reader = oio::StreamReader<CosReader>;
    type Writer = CosWriters;
    type Lister = CosListers;
    type Deleter = oio::OneShotDeleter<CosDeleter>;
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
        let resp = self.core.cos_head_object(ctx, path, &args).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let headers = resp.headers();
                let mut meta = parse_into_metadata(path, headers)?.into_builder();

                let user_meta = parse_prefixed_headers(headers, "x-cos-meta-");
                if !user_meta.is_empty() {
                    meta.user_metadata(user_meta);
                }

                if let Some(v) = parse_header_to_str(headers, constants::X_COS_VERSION_ID)?
                    && v != "null"
                {
                    meta.version(v);
                }

                Ok(RpStat::new(meta.build()))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("HeadObject"))
                    .with_caller_condition(args.is_conditional()),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<CosReader> = {
            Ok(oio::StreamReader::new(CosReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: CosWriters = {
            let writer = CosWriter::new(self.core.clone(), ctx.clone(), path, args.clone());

            let w = if args.append() {
                CosWriters::Two(oio::AppendWriter::new(writer))
            } else {
                CosWriters::One(oio::MultipartWriter::new(
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
        let output: oio::OneShotDeleter<CosDeleter> = {
            Ok(oio::OneShotDeleter::new(CosDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: CosListers = {
            let l = if args.versions() || args.deleted() {
                TwoWays::Two(oio::PageLister::new(CosObjectVersionsLister::new(
                    self.core.clone(),
                    ctx.clone(),
                    path,
                    args,
                )))
            } else {
                TwoWays::One(oio::PageLister::new(CosLister::new(
                    self.core.clone(),
                    ctx.clone(),
                    path,
                    args.recursive(),
                    args.limit(),
                )))
            };

            Ok(l)
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
        let core = self.core.clone();
        let ctx = ctx.clone();
        let from = from.to_string();
        let to = to.to_string();
        Ok(oio::OneShotCopier::new(async move {
            let source_size = match args.source_content_length_hint() {
                Some(size) => size,
                None => {
                    let stat_args = options::StatOptions {
                        version: args.source_version().map(str::to_owned),
                        ..Default::default()
                    }
                    .into();
                    let resp = core.cos_head_object(&ctx, &from, &stat_args).await?;
                    match resp.status() {
                        StatusCode::OK => {
                            parse_into_metadata(&from, resp.headers())?.content_length()
                        }
                        _ => {
                            return Err(parse_error(
                                ErrorContext::new(ServiceOperation("HeadObject")),
                                resp,
                            ));
                        }
                    }
                }
            };
            let resp = core.cos_copy_object(&ctx, &from, &to, &args).await?;

            let status = resp.status();

            match status {
                StatusCode::OK => Ok(MetadataBuilder::file(source_size).build()),
                _ => Err(parse_error(
                    ErrorContext::new(ServiceOperation("CopyObject"))
                        .with_if_not_exists(args.if_not_exists()),
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

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        if let Some(version) = args.version() {
            let copy_args = OpCopy::from_options(
                &self.capability(),
                options::CopyOptions {
                    source_version: Some(version.to_owned()),
                    ..Default::default()
                },
            )?;
            let resp = self
                .core
                .cos_copy_object(ctx, path, path, &copy_args)
                .await?;
            return match resp.status() {
                StatusCode::OK => Ok(RpRestore::new()),
                _ => Err(parse_error(
                    ErrorContext::new(ServiceOperation("CopyObject")),
                    resp,
                )),
            };
        }

        let resp = self
            .core
            .cos_list_object_versions(ctx, path, "", Some(1), "", "")
            .await?;
        if resp.status() != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListObjectVersions")),
                resp,
            ));
        }

        let output: ListObjectVersionsOutput =
            quick_xml::de::from_reader(resp.into_body().reader())
                .map_err(new_xml_deserialize_error)
                .map_err(Error::set_temporary)?;
        let abs_path = build_abs_path(&self.core.root, path);

        if output
            .version
            .iter()
            .any(|version| version.key == abs_path && version.is_latest)
        {
            return Ok(RpRestore::new());
        }

        let Some(marker) = output
            .delete_marker
            .into_iter()
            .find(|marker| marker.key == abs_path && marker.is_latest)
        else {
            return Err(Error::new(
                ErrorKind::NotFound,
                "no live object or current delete marker exists",
            ));
        };

        let delete_args = OpDelete::from_options(
            &self.capability(),
            options::DeleteOptions {
                version: Some(marker.version_id),
                ..Default::default()
            },
        )?;
        let resp = self.core.cos_delete_object(ctx, path, &delete_args).await?;
        match resp.status() {
            StatusCode::NO_CONTENT => {
                // COS can still list a marker after it was deleted. Only a
                // deletion of an existing marker returns this response header.
                if resp
                    .headers()
                    .get("x-cos-delete-marker")
                    .is_some_and(|v| v == "true")
                {
                    Ok(RpRestore::new())
                } else {
                    Err(Error::new(
                        ErrorKind::Unexpected,
                        "the listed delete marker no longer exists; retry after the version listing updates",
                    )
                    .set_temporary())
                }
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("DeleteObject")),
                resp,
            )),
        }
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        let req = match args.operation() {
            PresignOperation::Stat(v) => self.core.cos_head_object_request(path, v),
            PresignOperation::Read(range, v) => self.core.cos_get_object_request(path, *range, v),
            PresignOperation::Write(v) => {
                self.core
                    .cos_put_object_request(path, None, v, Buffer::new())
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

/// Compose the request endpoint for a bucket, as `scheme://bucket.host[:port]`.
///
/// Extracted so it can be unit tested, mirroring `S3Builder::build_endpoint`.
fn build_endpoint(uri: &Uri, bucket: &str) -> Result<String> {
    let scheme = uri.scheme_str().unwrap_or("https");

    let host = uri.host().ok_or_else(|| {
        Error::new(ErrorKind::ConfigInvalid, "endpoint host is empty")
            .with_context("service", COS_SCHEME)
            .with_context("endpoint", uri.to_string())
    })?;

    // If the endpoint already carries the bucket as its leftmost label, don't add it twice.
    let host = host.strip_prefix(&format!("{bucket}.")).unwrap_or(host);

    // Keep the port. `Uri::host` omits it, so composing from the host alone silently sent every
    // request to the scheme default.
    Ok(match uri.port_u16() {
        Some(port) => format!("{scheme}://{bucket}.{host}:{port}"),
        None => format!("{scheme}://{bucket}.{host}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint_of(raw: &str, bucket: &str) -> String {
        build_endpoint(&raw.parse::<Uri>().unwrap(), bucket).unwrap()
    }

    #[test]
    fn build_endpoint_keeps_a_custom_port() {
        assert_eq!(
            endpoint_of(
                "https://cos.internal.example.com:8443",
                "examplebucket-1250000000"
            ),
            "https://examplebucket-1250000000.cos.internal.example.com:8443"
        );
    }

    #[test]
    fn build_endpoint_prefixes_the_bucket() {
        assert_eq!(
            endpoint_of(
                "https://cos.ap-guangzhou.myqcloud.com",
                "examplebucket-1250000000"
            ),
            "https://examplebucket-1250000000.cos.ap-guangzhou.myqcloud.com"
        );
    }

    #[test]
    fn build_endpoint_does_not_repeat_a_bucket_already_in_the_host() {
        // The previous `replace("//{bucket}.", "//")` could never match, because Uri::host never
        // contains "//", so this doubled the bucket label.
        assert_eq!(
            endpoint_of(
                "https://examplebucket-1250000000.cos.ap-guangzhou.myqcloud.com",
                "examplebucket-1250000000"
            ),
            "https://examplebucket-1250000000.cos.ap-guangzhou.myqcloud.com"
        );
    }

    #[test]
    fn build_endpoint_defaults_the_scheme_to_https() {
        // A bare host parses with no scheme; "//host" parses with no host at all, which is why
        // the missing-host case below is a real input rather than a contrived one.
        assert_eq!(
            endpoint_of("cos.ap-guangzhou.myqcloud.com", "b"),
            "https://b.cos.ap-guangzhou.myqcloud.com"
        );
    }

    #[test]
    fn build_endpoint_reports_a_missing_host_instead_of_panicking() {
        // Both of these parse to host = None. The previous code called .unwrap() on that.
        for raw in ["/just/a/path", "//cos.ap-guangzhou.myqcloud.com"] {
            let uri = raw.parse::<Uri>().unwrap();
            assert!(
                build_endpoint(&uri, "b").is_err(),
                "expected an error for {raw}"
            );
        }
    }
}
