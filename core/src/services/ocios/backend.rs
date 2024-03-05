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
use std::fmt::Formatter;
use std::sync::Arc;

use async_trait::async_trait;
use http::StatusCode;
use log::debug;
use reqsign::OCIAPIKeySigner;
use reqsign::OCILoader;

use super::core::*;
use super::error::parse_error;
use super::lister::OciOsLister;
use crate::raw::*;
use crate::*;

/// Oracle Cloud Infrastructure Object Storage support
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct OciOsBuilder {
    root: Option<String>,

    region: String,
    namespace: String,
    endpoint: String,
    bucket: String,

    http_client: Option<HttpClient>,
}

impl Debug for OciOsBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("Builder");
        d.field("root", &self.root)
            .field("region", &self.region)
            .field("bucket", &self.bucket)
            .field("namespace", &self.namespace)
            .field("endpoint", &self.endpoint);

        d.finish_non_exhaustive()
    }
}

impl OciOsBuilder {
    /// Set root of this backend.
    ///
    /// All operations will happen under this root.
    pub fn root(&mut self, root: &str) -> &mut Self {
        self.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Set bucket name of this backend.
    pub fn bucket(&mut self, bucket: &str) -> &mut Self {
        self.bucket = bucket.to_string();

        self
    }

    /// Set namespace of this backend.
    pub fn namespace(&mut self, ns: &str) -> &mut Self {
        self.namespace = ns.to_string();

        self
    }

    /// Specify the http client that used by this service.
    ///
    /// # Notes
    ///
    /// This API is part of OpenDAL's Raw API. `HttpClient` could be changed
    /// during minor updates.
    pub fn http_client(&mut self, client: HttpClient) -> &mut Self {
        self.http_client = Some(client);
        self
    }
}

impl Builder for OciOsBuilder {
    const SCHEME: Scheme = Scheme::OciOs;
    type Accessor = OciOsBackend;

    fn from_map(_map: HashMap<String, String>) -> Self {
        OciOsBuilder::default()
    }

    fn build(&mut self) -> Result<Self::Accessor> {
        debug!("backend build started: {:?}", &self);

        let root = normalize_root(&self.root.clone().unwrap_or_default());
        debug!("backend use root {}", &root);

        let region = match self.region.is_empty() {
            false => Ok(&self.region),
            true => Err(
                Error::new(ErrorKind::ConfigInvalid, "The region is not set")
                    .with_context("service", Scheme::OciOs),
            ),
        }?;

        let bucket = match self.bucket.is_empty() {
            false => Ok(&self.bucket),
            true => Err(
                Error::new(ErrorKind::ConfigInvalid, "The bucket is misconfigured")
                    .with_context("service", Scheme::OciOs),
            ),
        }?;

        let namespace = match self.namespace.is_empty() {
            false => Ok(&self.namespace),
            true => Err(
                Error::new(ErrorKind::ConfigInvalid, "The namespace is not set")
                    .with_context("service", Scheme::OciOs),
            ),
        }?;

        let client = if let Some(client) = self.http_client.take() {
            client
        } else {
            HttpClient::new().map_err(|err| {
                err.with_operation("Builder::build")
                    .with_context("service", Scheme::OciOs)
            })?
        };

        let loader = OCILoader::default();

        let signer = OCIAPIKeySigner::default();

        debug!("Backend build finished");

        Ok(OciOsBackend {
            core: Arc::new(OciOsCore {
                region: region.to_owned(),
                root,
                bucket: bucket.to_owned(),
                namespace: namespace.to_owned(),
                signer,
                loader,
                client,
            }),
        })
    }
}

#[derive(Debug, Clone)]
/// Oracle Cloud Infrastructure Object Storage Service backend
pub struct OciOsBackend {
    core: Arc<OciOsCore>,
}

#[async_trait]
impl Accessor for OciOsBackend {
    type Reader = IncomingAsyncBody;
    type Lister = oio::PageLister<OciOsLister>;
    type Writer = ();
    type BlockingReader = ();
    type BlockingWriter = ();
    type BlockingLister = ();

    fn info(&self) -> AccessorInfo {
        let mut am = AccessorInfo::default();
        am.set_scheme(Scheme::OciOs)
            .set_root(&self.core.root)
            .set_name(&self.core.bucket)
            .set_native_capability(Capability {
                stat: true,
                stat_with_if_match: true,
                stat_with_if_none_match: true,

                read: true,
                read_can_next: true,
                read_with_range: true,
                read_with_if_match: true,
                read_with_if_none_match: true,

                write: false,

                delete: false,
                copy: false,

                list: true,
                list_with_limit: true,
                list_with_start_after: true,
                list_with_recursive: true,

                presign: false,
                presign_stat: true,
                presign_read: true,
                presign_write: true,

                batch: false,

                ..Default::default()
            });

        am
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let resp = self
            .core
            .os_get_object(
                path,
                args.range(),
                args.if_match(),
                args.if_none_match(),
                args.override_content_disposition(),
            )
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                let size = parse_content_length(resp.headers())?;
                let range = parse_content_range(resp.headers())?;
                Ok((
                    RpRead::new().with_size(size).with_range(range),
                    resp.into_body(),
                ))
            }
            StatusCode::RANGE_NOT_SATISFIABLE => {
                resp.into_body().consume().await?;
                Ok((RpRead::new().with_size(Some(0)), IncomingAsyncBody::empty()))
            }
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let resp = self
            .core
            .os_head_object(path, args.if_match(), args.if_none_match())
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => parse_into_metadata(path, resp.headers()).map(RpStat::new),
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        let l = OciOsLister::new(
            self.core.clone(),
            path,
            args.recursive(),
            args.limit(),
            args.start_after(),
        );
        Ok((RpList::default(), oio::PageLister::new(l)))
    }
}
