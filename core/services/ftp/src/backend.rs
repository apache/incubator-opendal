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
use std::str::FromStr;
use std::sync::Arc;

use http::Uri;
use log::debug;
use suppaftp::FtpError;
use suppaftp::Status;
use suppaftp::list::File;
use suppaftp::types::Response;

use super::FTP_SCHEME;
use super::config::FtpConfig;
use super::core::FtpCore;
use super::core::Manager;
use super::core::format_ftp_error;
use super::deleter::FtpDeleter;
use super::reader::*;
use opendal_core::raw::*;
use opendal_core::*;

/// FTP and FTPS services support.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct FtpBuilder {
    pub(super) config: FtpConfig,
}

impl FtpBuilder {
    /// set endpoint for ftp backend.
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.config.endpoint = if endpoint.is_empty() {
            None
        } else {
            Some(endpoint.to_string())
        };

        self
    }

    /// set root path for ftp backend.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// set user for ftp backend.
    pub fn user(mut self, user: &str) -> Self {
        self.config.user = if user.is_empty() {
            None
        } else {
            Some(user.to_string())
        };

        self
    }

    /// set password for ftp backend.
    pub fn password(mut self, password: &str) -> Self {
        self.config.password = if password.is_empty() {
            None
        } else {
            Some(password.to_string())
        };

        self
    }
}

impl Builder for FtpBuilder {
    type Config = FtpConfig;

    fn build(self) -> Result<impl Service> {
        debug!("ftp backend build started: {:?}", self);
        let endpoint = match &self.config.endpoint {
            None => return Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")),
            Some(v) => v,
        };

        let endpoint_uri = match endpoint.parse::<Uri>() {
            Err(e) => {
                return Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is invalid")
                    .with_context("endpoint", endpoint)
                    .set_source(e));
            }
            Ok(uri) => uri,
        };

        let host = endpoint_uri.host().unwrap_or("127.0.0.1");
        let port = endpoint_uri.port_u16().unwrap_or(21);

        let endpoint = format!("{host}:{port}");

        let enable_secure = match endpoint_uri.scheme_str() {
            Some("ftp") => false,
            // if the user forgot to add a scheme prefix
            // treat it as using secured scheme
            Some("ftps") | None => true,

            Some(s) => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "endpoint is unsupported or invalid",
                )
                .with_context("endpoint", s));
            }
        };

        let root = normalize_root(&self.config.root.unwrap_or_default());

        let user = match &self.config.user {
            None => "".to_string(),
            Some(v) => v.clone(),
        };

        let password = match &self.config.password {
            None => "".to_string(),
            Some(v) => v.clone(),
        };

        let info = ServiceInfo::new(FTP_SCHEME, &root, "");
        let capability = Capability {
            stat: true,

            read: true,

            write: true,
            write_can_multi: true,
            write_can_append: true,

            delete: true,
            create_dir: true,

            list: true,

            shared: true,

            ..Default::default()
        };

        let manager = Manager {
            endpoint: endpoint.clone(),
            host: host.to_string(),
            root: root.clone(),
            user: user.clone(),
            password: password.clone(),
            enable_secure,
        };

        let core = Arc::new(FtpCore::new(info, capability, manager.clone()));
        Ok(FtpBackend { core })
    }
}

#[derive(Clone)]
pub struct FtpBackend {
    pub(crate) core: Arc<FtpCore>,
}

impl Debug for FtpBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FtpBackend").finish()
    }
}

impl Service for FtpBackend {
    type Reader = oio::StreamReader<FtpReader>;
    type Writer = FtpLazyWriter;
    type Lister = FtpLazyLister;
    type Deleter = oio::OneShotDeleter<FtpDeleter>;
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.core.info()
    }

    fn capability(&self) -> Capability {
        self.core.capability()
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        let mut ftp_stream = self.core.ftp_connect(Operation::CreateDir).await?;

        let paths: Vec<&str> = path.split_inclusive('/').collect();

        let mut curr_path = String::new();

        for path in paths {
            curr_path.push_str(path);
            match ftp_stream.mkdir(&curr_path).await {
                // Do nothing if status is FileUnavailable or OK(()) is return.
                Err(FtpError::UnexpectedResponse(Response {
                    status: Status::FileUnavailable,
                    ..
                }))
                | Ok(()) => (),
                Err(e) => {
                    return Err(format_ftp_error(e));
                }
            }
        }

        Ok(RpCreateDir::default())
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let file = self.ftp_stat(path).await?;

        let mode = if file.is_file() {
            EntryMode::FILE
        } else if file.is_directory() {
            EntryMode::DIR
        } else {
            EntryMode::Unknown
        };

        let mut meta = match mode {
            EntryMode::FILE => MetadataBuilder::file(file.size() as u64),
            EntryMode::DIR => MetadataBuilder::dir(),
            EntryMode::Unknown => MetadataBuilder::unknown(),
        };
        meta.last_modified(Timestamp::try_from(file.modified())?);

        Ok(RpStat::new(meta.build()))
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<FtpReader> = {
            Ok(oio::StreamReader::new(FtpReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, op: OpWrite) -> Result<Self::Writer> {
        Ok(FtpLazyWriter::new(self.core.clone(), path, op))
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<FtpDeleter> =
            { Ok(oio::OneShotDeleter::new(FtpDeleter::new(self.core.clone()))) }?;

        Ok(output)
    }

    fn list(&self, _ctx: &OperationContext, path: &str, _: OpList) -> Result<Self::Lister> {
        Ok(FtpLazyLister::new(self.core.clone(), path))
    }

    fn copy(
        &self,
        _ctx: &OperationContext,
        _from: &str,
        _to: &str,
        _args: OpCopy,
    ) -> Result<Self::Copier> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
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
        _ctx: &OperationContext,
        _path: &str,
        _args: OpPresign,
    ) -> Result<RpPresign> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }
}

impl FtpBackend {
    pub async fn ftp_stat(&self, path: &str) -> Result<File> {
        let mut ftp_stream = self.core.ftp_connect(Operation::Stat).await?;

        let (parent, basename) = (get_parent(path), get_basename(path));

        let pathname = if parent == "/" { None } else { Some(parent) };

        let resp = ftp_stream.list(pathname).await.map_err(format_ftp_error)?;

        // Get stat of file.
        let mut files = resp
            .into_iter()
            .filter_map(|file| File::from_str(file.as_str()).ok())
            .filter(|f| f.name() == basename.trim_end_matches('/'))
            .collect::<Vec<File>>();

        if files.is_empty() {
            Err(Error::new(
                ErrorKind::NotFound,
                "file is not found during list",
            ))
        } else {
            Ok(files.remove(0))
        }
    }
}

#[cfg(test)]
mod build_test {
    use super::FtpBuilder;
    use crate::FtpConfig;
    use opendal_core::*;

    #[test]
    fn test_build() {
        // ftps scheme, should suffix with default port 21
        let b = FtpBuilder::default()
            .endpoint("ftps://ftp_server.local")
            .build();
        assert!(b.is_ok());

        // ftp scheme
        let b = FtpBuilder::default()
            .endpoint("ftp://ftp_server.local:1234")
            .build();
        assert!(b.is_ok());

        // no scheme
        let b = FtpBuilder::default()
            .endpoint("ftp_server.local:8765")
            .build();
        assert!(b.is_ok());

        // invalid scheme
        let b = FtpBuilder::default()
            .endpoint("invalidscheme://ftp_server.local:8765")
            .build();
        assert!(b.is_err());
        let e = b.unwrap_err();
        assert_eq!(e.kind(), ErrorKind::ConfigInvalid);
    }

    #[test]
    fn from_uri_sets_endpoint_and_root() {
        let uri = OperatorUri::new(
            "ftp://example.com/public/data",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = FtpConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoint.as_deref(), Some("ftp://example.com"));
        assert_eq!(cfg.root.as_deref(), Some("public/data"));
    }

    #[test]
    fn from_uri_applies_credentials_from_query() {
        let uri = OperatorUri::new(
            "ftp://example.com/data",
            vec![
                ("user".to_string(), "alice".to_string()),
                ("password".to_string(), "secret".to_string()),
            ],
        )
        .unwrap();

        let cfg = FtpConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoint.as_deref(), Some("ftp://example.com"));
        assert_eq!(cfg.user.as_deref(), Some("alice"));
        assert_eq!(cfg.password.as_deref(), Some("secret"));
    }
}
