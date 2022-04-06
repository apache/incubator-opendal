// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;
use std::io::ErrorKind;
use std::io::Result;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::anyhow;
use async_compat::Compat;
use async_trait::async_trait;
use futures::AsyncReadExt;
use futures::AsyncSeekExt;
use log::debug;
use log::error;
use log::info;
use metrics::increment_counter;
use minitrace::trace;
use time::OffsetDateTime;
use tokio::fs;

use super::error::parse_io_error;
use super::object_stream::Readdir;
use crate::error::other;
use crate::error::BackendError;
use crate::error::ObjectError;
use crate::object::Metadata;
use crate::object::ObjectMode;
use crate::object::ObjectStreamer;
use crate::ops::OpCreate;
use crate::ops::OpDelete;
use crate::ops::OpList;
use crate::ops::OpRead;
use crate::ops::OpStat;
use crate::ops::OpWrite;
use crate::Accessor;
use crate::BytesReader;
use crate::BytesWriter;

#[derive(Default, Debug)]
pub struct Builder {
    root: Option<String>,
}

impl Builder {
    pub fn root(&mut self, root: &str) -> &mut Self {
        self.root = Some(root.to_string());

        self
    }

    pub async fn finish(&mut self) -> Result<Arc<dyn Accessor>> {
        info!("backend build started: {:?}", &self);

        // Make `/` as the default of root.
        let root = match &self.root {
            None => "/".to_string(),
            Some(v) => {
                if !v.starts_with('/') {
                    return Err(other(BackendError::new(
                        HashMap::from([("root".to_string(), v.clone())]),
                        anyhow!("Root must start with /"),
                    )));
                }
                v.to_string()
            }
        };

        // If root dir is not exist, we must create it.
        if let Err(e) = fs::metadata(&root).await {
            if e.kind() == std::io::ErrorKind::NotFound {
                fs::create_dir_all(&root)
                    .await
                    .map_err(|e| parse_io_error(e, "build", &root))?;
            }
        }

        info!("backend build finished: {:?}", &self);
        Ok(Arc::new(Backend { root }))
    }
}

/// Backend is used to serve `Accessor` support for posix alike fs.
#[derive(Debug, Clone)]
pub struct Backend {
    root: String,
}

impl Backend {
    pub fn build() -> Builder {
        Builder::default()
    }

    pub(crate) fn get_abs_path(&self, path: &str) -> String {
        if path == "/" {
            return self.root.clone();
        }

        PathBuf::from(&self.root)
            .join(path)
            .to_string_lossy()
            .to_string()
    }
}

#[async_trait]
impl Accessor for Backend {
    async fn create(&self, args: &OpCreate) -> Result<()> {
        let path = self.get_abs_path(args.path());

        let parent = PathBuf::from(&path)
            .parent()
            .ok_or_else(|| {
                other(ObjectError::new(
                    "write",
                    &path,
                    anyhow!("malformed path: {:?}", &path),
                ))
            })?
            .to_path_buf();

        fs::create_dir_all(&parent).await.map_err(|e| {
            let e = parse_io_error(e, "write", &parent.to_string_lossy());
            error!(
                "object {} create_dir_all for parent {}: {:?}",
                &path,
                &parent.to_string_lossy(),
                e
            );
            e
        })?;

        if args.mode() == ObjectMode::FILE {
            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&path)
                .await
                .map_err(|e| {
                    let e = parse_io_error(e, "write", &path);
                    error!("object {} create: {:?}", &path, e);
                    e
                })?;

            return Ok(());
        }

        if args.mode() == ObjectMode::DIR {
            fs::create_dir(&path).await.map_err(|e| {
                let e = parse_io_error(e, "write", &path);
                error!("object {} create: {:?}", &path, e);
                e
            })?;

            return Ok(());
        }

        unreachable!()
    }

    #[trace("read")]
    async fn read(&self, args: &OpRead) -> Result<BytesReader> {
        increment_counter!("opendal_fs_read_requests");

        let path = self.get_abs_path(args.path());
        debug!(
            "object {} read start: offset {:?}, size {:?}",
            &path,
            args.offset(),
            args.size()
        );

        let f = fs::OpenOptions::new()
            .read(true)
            .open(&path)
            .await
            .map_err(|e| {
                let e = parse_io_error(e, "read", &path);
                error!("object {} open: {:?}", &path, e);
                e
            })?;

        let mut f = Compat::new(f);

        if let Some(offset) = args.offset() {
            f.seek(SeekFrom::Start(offset)).await.map_err(|e| {
                let e = parse_io_error(e, "read", &path);
                error!("object {} seek: {:?}", &path, e);
                e
            })?;
        };

        let r: BytesReader = match args.size() {
            Some(size) => Box::new(f.take(size)),
            None => Box::new(f),
        };

        debug!(
            "object {} reader created: offset {:?}, size {:?}",
            &path,
            args.offset(),
            args.size()
        );
        Ok(Box::new(r))
    }

    #[trace("write")]
    async fn write(&self, args: &OpWrite) -> Result<BytesWriter> {
        increment_counter!("opendal_fs_write_requests");

        let path = self.get_abs_path(args.path());
        debug!("object {} write start: size {}", &path, args.size());

        // Create dir before write path.
        //
        // TODO(xuanwo): There are many works to do here:
        //   - Is it safe to create dir concurrently?
        //   - Do we need to extract this logic as new util functions?
        //   - Is it better to check the parent dir exists before call mkdir?
        let parent = PathBuf::from(&path)
            .parent()
            .ok_or_else(|| {
                other(ObjectError::new(
                    "write",
                    &path,
                    anyhow!("malformed path: {:?}", &path),
                ))
            })?
            .to_path_buf();

        fs::create_dir_all(&parent).await.map_err(|e| {
            let e = parse_io_error(e, "write", &parent.to_string_lossy());
            error!(
                "object {} create_dir_all for parent {}: {:?}",
                &path,
                &parent.to_string_lossy(),
                e
            );
            e
        })?;

        let f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&path)
            .await
            .map_err(|e| {
                let e = parse_io_error(e, "write", &path);
                error!("object {} open: {:?}", &path, e);
                e
            })?;

        debug!("object {} write finished: size {:?}", &path, args.size());
        Ok(Box::new(Compat::new(f)))
    }

    #[trace("stat")]
    async fn stat(&self, args: &OpStat) -> Result<Metadata> {
        increment_counter!("opendal_fs_stat_requests");

        let path = self.get_abs_path(args.path());
        debug!("object {} stat start", &path);

        let meta = fs::metadata(&path).await.map_err(|e| {
            let e = parse_io_error(e, "stat", &path);
            error!("object {} stat: {:?}", &path, e);
            e
        })?;

        let mut m = Metadata::default();
        if meta.is_dir() {
            let mut p = args.path().to_string();
            if !p.ends_with('/') {
                p.push('/')
            }
            m.set_path(&p);
            m.set_mode(ObjectMode::DIR);
        } else if meta.is_file() {
            m.set_path(args.path());
            m.set_mode(ObjectMode::FILE);
        } else {
            m.set_path(args.path());
            m.set_mode(ObjectMode::Unknown);
        }
        m.set_content_length(meta.len() as u64);
        m.set_last_modified(
            meta.modified()
                .map(OffsetDateTime::from)
                .map_err(|e| parse_io_error(e, "stat", &path))?,
        );
        m.set_complete();

        debug!("object {} stat finished", &path);
        Ok(m)
    }

    #[trace("delete")]
    async fn delete(&self, args: &OpDelete) -> Result<()> {
        increment_counter!("opendal_fs_delete_requests");

        let path = self.get_abs_path(&args.path);
        debug!("object {} delete start", &path);

        // PathBuf.is_dir() is not free, call metadata directly instead.
        let meta = fs::metadata(&path).await;

        if let Err(err) = meta {
            return if err.kind() == ErrorKind::NotFound {
                Ok(())
            } else {
                let e = parse_io_error(err, "delete", &path);
                error!("object {} delete: {:?}", &path, e);
                Err(e)
            };
        }

        // Safety: Err branch has been checked, it's OK to unwrap.
        let meta = meta.ok().unwrap();

        let f = if meta.is_dir() {
            fs::remove_dir(&path).await
        } else {
            fs::remove_file(&path).await
        };

        f.map_err(|e| parse_io_error(e, "delete", &path))?;

        debug!("object {} delete finished", &path);
        Ok(())
    }

    #[trace("list")]
    async fn list(&self, args: &OpList) -> Result<ObjectStreamer> {
        increment_counter!("opendal_fs_list_requests");

        let path = self.get_abs_path(&args.path);
        debug!("object {} list start", &path);

        let f = std::fs::read_dir(&path).map_err(|e| {
            let e = parse_io_error(e, "list", &path);
            error!("object {} list: {:?}", &path, e);
            e
        })?;

        let rd = Readdir::new(Arc::new(self.clone()), &self.root, &args.path, f);

        Ok(Box::new(rd))
    }
}
