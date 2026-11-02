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

use flutter_rust_bridge::frb;

use ::opendal as od;

use std::collections::HashMap;
use std::sync::LazyLock;

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

#[frb(opaque)]
pub struct Operator {
    async_op: opendal::Operator,
    blocking_op: opendal::blocking::Operator,
}

impl Operator {
    #[frb(sync)]
    pub fn new(scheme: String, map: HashMap<String, String>) -> Operator {
        let async_op = od::Operator::via_iter(scheme, map).unwrap();
        let handle = RUNTIME.handle();
        let _enter = handle.enter();
        let blocking_op = opendal::blocking::Operator::new(async_op.clone()).unwrap();

        Self {
            async_op,
            blocking_op,
        }
    }
    pub async fn stat(&self, path: String) -> Metadata {
        let meta = self.async_op.stat(&path).await.unwrap();

        Metadata(meta)
    }
    #[frb(sync)]
    pub fn stat_sync(&self, path: String) -> Metadata {
        let meta = self.blocking_op.stat(&path).unwrap();

        Metadata(meta)
    }
    pub async fn check(&self) {
        self.async_op.check().await.unwrap()
    }
    pub async fn exists(&self, path: String) -> bool {
        self.async_op.exists(&path).await.unwrap()
    }
    pub async fn delete(&self, path: String) {
        self.async_op.delete(&path).await.unwrap()
    }
    #[frb(sync)]
    pub fn delete_sync(&self, path: String) -> () {
        self.blocking_op.delete(&path).unwrap()
    }
    #[frb(sync)]
    pub fn exists_sync(&self, path: String) -> bool {
        self.blocking_op.exists(&path).unwrap()
    }
    pub async fn create_dir(&self, path: String) {
        self.async_op.create_dir(&path).await.unwrap()
    }
    #[frb(sync)]
    pub fn create_dir_sync(&self, path: String) -> () {
        self.blocking_op.create_dir(&path).unwrap()
    }
    pub async fn rename(&self, from: String, to: String) {
        self.async_op.rename(&from, &to).await.unwrap()
    }
    #[frb(sync)]
    pub fn rename_sync(&self, from: String, to: String) -> () {
        self.blocking_op.rename(&from, &to).unwrap()
    }

    pub async fn read(&self, path: String) -> anyhow::Result<Vec<u8>> {
        let buf = self
            .async_op
            .read(&path)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(buf.to_vec())
    }

    #[frb(sync)]
    pub fn read_sync(&self, path: String) -> anyhow::Result<Vec<u8>> {
        let buf = self
            .blocking_op
            .read(&path)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(buf.to_vec())
    }

    pub async fn write(&self, path: String, data: Vec<u8>) -> anyhow::Result<()> {
        self.async_op
            .write(&path, data)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }

    #[frb(sync)]
    pub fn write_sync(&self, path: String, data: Vec<u8>) -> anyhow::Result<()> {
        self.blocking_op
            .write(&path, data)
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
}

#[frb(opaque)]
pub struct Metadata(opendal::Metadata);

impl Metadata {
    /// Returns true if the <op.stat> object describes a file system directory.
    #[frb(sync, getter)]
    pub fn is_directory(&self) -> bool {
        self.0.is_dir()
    }

    /// Returns true if the <op.stat> object describes a regular file.
    #[frb(sync, getter)]
    pub fn is_file(&self) -> bool {
        self.0.is_file()
    }

    /// Content-Disposition of this object
    #[frb(sync, getter)]
    pub fn content_disposition(&self) -> Option<String> {
        self.0.content_disposition().map(|s| s.to_string())
    }

    /// Content Length of this object
    #[frb(sync, getter)]
    pub fn content_length(&self) -> Option<u64> {
        self.0.content_length().into()
    }

    /// Content MD5 of this object.
    #[frb(sync, getter)]
    pub fn content_md5(&self) -> Option<String> {
        self.0.content_md5().map(|s| s.to_string())
    }

    /// Content Type of this object.
    #[frb(sync, getter)]
    pub fn content_type(&self) -> Option<String> {
        self.0.content_type().map(|s| s.to_string())
    }

    /// ETag of this object.
    #[frb(sync, getter)]
    pub fn etag(&self) -> Option<String> {
        self.0.etag().map(|s| s.to_string())
    }

    /// Last Modified of this object.
    ///
    /// We will output this time in RFC3339 format like `1996-12-19T16:39:57+08:00`.
    #[frb(sync, getter)]
    pub fn last_modified(&self) -> Option<String> {
        self.0.last_modified().map(|ta| ta.to_string())
    }
}
