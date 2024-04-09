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
use std::vec::IntoIter;

use async_trait::async_trait;
use bytes::{Buf, Bytes};

use super::Adapter;
use super::Value;
use crate::raw::oio::HierarchyLister;
use crate::raw::*;
use crate::*;

/// The typed kv backend which implements Accessor for typed kv adapter.
#[derive(Debug, Clone)]
pub struct Backend<S: Adapter> {
    kv: Arc<S>,
    root: String,
}

impl<S> Backend<S>
where
    S: Adapter,
{
    /// Create a new kv backend.
    pub fn new(kv: S) -> Self {
        Self {
            kv: Arc::new(kv),
            root: "/".to_string(),
        }
    }

    /// Configure root within this backend.
    pub fn with_root(mut self, root: &str) -> Self {
        self.root = normalize_root(root);
        self
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S: Adapter> Accessor for Backend<S> {
    type Reader = Bytes;
    type BlockingReader = Bytes;
    type Writer = KvWriter<S>;
    type BlockingWriter = KvWriter<S>;
    type Lister = HierarchyLister<KvLister>;
    type BlockingLister = HierarchyLister<KvLister>;

    fn info(&self) -> AccessorInfo {
        let kv_info = self.kv.info();
        let mut am: AccessorInfo = AccessorInfo::default();
        am.set_root(&self.root);
        am.set_scheme(kv_info.scheme());
        am.set_name(kv_info.name());

        let kv_cap = kv_info.capabilities();
        let mut cap = Capability::default();
        if kv_cap.get {
            cap.read = true;
            cap.stat = true;
        }

        if kv_cap.set {
            cap.write = true;
            cap.write_can_empty = true;
        }

        if kv_cap.delete {
            cap.delete = true;
        }

        if kv_cap.scan {
            cap.list = true;
            cap.list_with_recursive = true;
        }

        cap.blocking = true;

        am.set_native_capability(cap);

        am
    }

    async fn read(&self, path: &str, _: OpRead) -> Result<(RpRead, Self::Reader)> {
        let p = build_abs_path(&self.root, path);

        let bs = match self.kv.get(&p).await? {
            // TODO: we can reuse the metadata in value to build content range.
            Some(bs) => bs.value,
            None => return Err(Error::new(ErrorKind::NotFound, "kv doesn't have this path")),
        };

        Ok((RpRead::new(), bs))
    }

    fn blocking_read(&self, path: &str, _: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        let p = build_abs_path(&self.root, path);

        let bs = match self.kv.blocking_get(&p)? {
            // TODO: we can reuse the metadata in value to build content range.
            Some(bs) => bs.value,
            None => return Err(Error::new(ErrorKind::NotFound, "kv doesn't have this path")),
        };

        Ok((RpRead::new(), bs))
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        let p = build_abs_path(&self.root, path);

        Ok((RpWrite::new(), KvWriter::new(self.kv.clone(), p, args)))
    }

    fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        let p = build_abs_path(&self.root, path);

        Ok((RpWrite::new(), KvWriter::new(self.kv.clone(), p, args)))
    }

    async fn stat(&self, path: &str, _: OpStat) -> Result<RpStat> {
        let p = build_abs_path(&self.root, path);

        if p == build_abs_path(&self.root, "") {
            Ok(RpStat::new(Metadata::new(EntryMode::DIR)))
        } else {
            let bs = self.kv.get(&p).await?;
            match bs {
                Some(bs) => Ok(RpStat::new(bs.metadata)),
                None => Err(Error::new(ErrorKind::NotFound, "kv doesn't have this path")),
            }
        }
    }

    fn blocking_stat(&self, path: &str, _: OpStat) -> Result<RpStat> {
        let p = build_abs_path(&self.root, path);

        if p == build_abs_path(&self.root, "") {
            Ok(RpStat::new(Metadata::new(EntryMode::DIR)))
        } else {
            let bs = self.kv.blocking_get(&p)?;
            match bs {
                Some(bs) => Ok(RpStat::new(bs.metadata)),
                None => Err(Error::new(ErrorKind::NotFound, "kv doesn't have this path")),
            }
        }
    }

    async fn delete(&self, path: &str, _: OpDelete) -> Result<RpDelete> {
        let p = build_abs_path(&self.root, path);

        self.kv.delete(&p).await?;
        Ok(RpDelete::default())
    }

    fn blocking_delete(&self, path: &str, _: OpDelete) -> Result<RpDelete> {
        let p = build_abs_path(&self.root, path);

        self.kv.blocking_delete(&p)?;
        Ok(RpDelete::default())
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        let p = build_abs_path(&self.root, path);
        let res = self.kv.scan(&p).await?;
        let lister = KvLister::new(&self.root, res);
        let lister = HierarchyLister::new(lister, path, args.recursive());

        Ok((RpList::default(), lister))
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
        let p = build_abs_path(&self.root, path);
        let res = self.kv.blocking_scan(&p)?;
        let lister = KvLister::new(&self.root, res);
        let lister = HierarchyLister::new(lister, path, args.recursive());

        Ok((RpList::default(), lister))
    }
}

pub struct KvLister {
    root: String,
    inner: IntoIter<String>,
}

impl KvLister {
    fn new(root: &str, inner: Vec<String>) -> Self {
        Self {
            root: root.to_string(),
            inner: inner.into_iter(),
        }
    }

    fn inner_next(&mut self) -> Option<oio::Entry> {
        self.inner.next().map(|v| {
            let mode = if v.ends_with('/') {
                EntryMode::DIR
            } else {
                EntryMode::FILE
            };

            oio::Entry::new(&build_rel_path(&self.root, &v), Metadata::new(mode))
        })
    }
}

impl oio::List for KvLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        Ok(self.inner_next())
    }
}

impl oio::BlockingList for KvLister {
    fn next(&mut self) -> Result<Option<oio::Entry>> {
        Ok(self.inner_next())
    }
}

pub struct KvWriter<S> {
    kv: Arc<S>,
    path: String,

    op: OpWrite,
    buf: Option<Vec<u8>>,
    value: Option<Value>,
}

/// # Safety
///
/// We will only take `&mut Self` reference for KvWriter.
unsafe impl<S: Adapter> Sync for KvWriter<S> {}

impl<S> KvWriter<S> {
    fn new(kv: Arc<S>, path: String, op: OpWrite) -> Self {
        KvWriter {
            kv,
            path,
            op,
            buf: None,
            value: None,
        }
    }

    fn build(&mut self) -> Value {
        let value = self.buf.take().map(Bytes::from).unwrap_or_default();

        let mut metadata = Metadata::new(EntryMode::FILE);
        metadata.set_content_length(value.len() as u64);

        if let Some(v) = self.op.cache_control() {
            metadata.set_cache_control(v);
        }
        if let Some(v) = self.op.content_disposition() {
            metadata.set_content_disposition(v);
        }
        if let Some(v) = self.op.content_type() {
            metadata.set_content_type(v);
        }

        Value { metadata, value }
    }
}

impl<S: Adapter> oio::Write for KvWriter<S> {
    async fn write(&mut self, bs: oio::Buffer) -> Result<usize> {
        let size = bs.chunk().len();

        let mut buf = self.buf.take().unwrap_or_else(|| Vec::with_capacity(size));
        buf.extend_from_slice(bs.chunk());

        self.buf = Some(buf);
        Ok(size)
    }

    async fn close(&mut self) -> Result<()> {
        let value = match &self.value {
            Some(value) => value.clone(),
            None => {
                let value = self.build();
                self.value = Some(value.clone());
                value
            }
        };
        self.kv.set(&self.path, value).await?;
        Ok(())
    }

    async fn abort(&mut self) -> Result<()> {
        self.buf = None;
        Ok(())
    }
}

impl<S: Adapter> oio::BlockingWrite for KvWriter<S> {
    fn write(&mut self, bs: oio::Buffer) -> Result<usize> {
        let size = bs.len();

        let mut buf = self.buf.take().unwrap_or_else(|| Vec::with_capacity(size));
        buf.extend_from_slice(bs.chunk());

        self.buf = Some(buf);

        Ok(size)
    }

    fn close(&mut self) -> Result<()> {
        let kv = self.kv.clone();
        let value = match &self.value {
            Some(value) => value.clone(),
            None => {
                let value = self.build();
                self.value = Some(value.clone());
                value
            }
        };

        kv.blocking_set(&self.path, value)?;
        Ok(())
    }
}
