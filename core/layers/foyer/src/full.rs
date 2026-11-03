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

use opendal_core::Buffer;
use opendal_core::BytesRange;
use opendal_core::Error;
use opendal_core::MetadataBuilder;
use opendal_core::Result;
use opendal_core::options;
use opendal_core::raw::OpRead;
use opendal_core::raw::OpStat;
use opendal_core::raw::RpRead;
use opendal_core::raw::Service;
use opendal_core::raw::oio;
use opendal_core::raw::oio::Read as _;
use opendal_core::raw::oio::ReadStream;

use crate::FoyerKey;
use crate::FoyerValue;
use crate::Inner;
use crate::error::{FetchError, extract_err};

pub struct FullReader {
    inner: Arc<Inner>,
    size_limit: std::ops::Range<usize>,
    path: String,
    args: OpRead,
}

impl FullReader {
    pub(crate) fn new(
        inner: Arc<Inner>,
        size_limit: std::ops::Range<usize>,
        path: String,
        args: OpRead,
    ) -> Self {
        Self {
            inner,
            size_limit,
            path,
            args,
        }
    }

    fn cache_key(&self) -> FoyerKey {
        FoyerKey {
            path: self.path.clone(),
            version: self.args.version().map(|v| v.to_string()),
        }
    }

    fn full_object_rp(buffer: &Buffer) -> RpRead {
        RpRead::new({
            let metadata = MetadataBuilder::file(buffer.len() as _);
            metadata.build()
        })
    }

    fn slice_full_object(buffer: &Buffer, range: BytesRange) -> Buffer {
        buffer.slice(range.to_range_as_usize())
    }

    fn is_deleted(&self, key: &FoyerKey) -> bool {
        self.inner
            .deleted_keys
            .lock()
            .expect("deleted keys lock poisoned")
            .contains(key)
    }

    /// Derive the `OpStat` used for cache fill from the reader's `OpRead`, so the
    /// stat observes the same conditions (version, if-match, ...) as the request.
    fn stat_args(&self) -> OpStat {
        options::StatOptions {
            version: self.args.version().map(str::to_owned),
            if_match: self.args.if_match().map(str::to_owned),
            if_none_match: self.args.if_none_match().map(str::to_owned),
            if_version_match: self.args.if_version_match().map(str::to_owned),
            if_version_not_match: self.args.if_version_not_match().map(str::to_owned),
            if_modified_since: self.args.if_modified_since(),
            if_unmodified_since: self.args.if_unmodified_since(),
            ..Default::default()
        }
        .into()
    }

    async fn fallback_open(
        &self,
        range: BytesRange,
    ) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let reader = self
            .inner
            .srv
            .read(&self.inner.ctx, &self.path, self.args.clone())?;
        reader.open(range).await
    }

    async fn fallback_read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let reader = self
            .inner
            .srv
            .read(&self.inner.ctx, &self.path, self.args.clone())?;
        reader.read(range).await
    }

    async fn read_full_object(&self) -> Result<Option<Buffer>> {
        let path_str = self.path.clone();

        // Use get_or_fetch to read data from cache or fallback to remote. It can automatically
        // handle the thundering herd problem by ensuring only one request is made for a given
        // key.
        //
        // Please note that we only cache the object if it's smaller than size_limit. And we'll
        // fetch the ENTIRE object from remote to put it into cache, then slice it to the requested
        // range.
        let key = self.cache_key();

        if self.is_deleted(&key) {
            return Ok(None);
        }

        let result = self
            .inner
            .cache
            .get_or_fetch(&key, || {
                let inner = self.inner.clone();
                let size_limit = self.size_limit.clone();
                let path_clone = path_str.clone();
                let stat_args = self.stat_args();
                let read_args = self.args.clone();
                async move {
                    // read the metadata first, if it's too large, do not cache
                    let metadata = inner
                        .srv
                        .stat(&inner.ctx, &path_clone, stat_args)
                        .await
                        .map_err(FetchError::from_error)?
                        .into_metadata();

                    let size = metadata.content_length() as usize;
                    if !size_limit.contains(&size) {
                        return Err(FetchError::SizeTooLarge);
                    }

                    // fetch the ENTIRE object from remote.
                    let reader = inner
                        .srv
                        .read(&inner.ctx, &path_clone, read_args)
                        .map_err(FetchError::from_error)?;
                    let (_, mut stream) = reader
                        .open(BytesRange::new(0, None))
                        .await
                        .map_err(FetchError::from_error)?;
                    let buffer = stream.read_all().await.map_err(FetchError::from_error)?;

                    Ok(FoyerValue(buffer))
                }
            })
            .await;

        match result {
            Ok(entry) => Ok(Some(entry.0.clone())),
            Err(e) => match e.downcast_ref::<FetchError>() {
                Some(FetchError::SizeTooLarge) => Ok(None),
                Some(FetchError::Source { kind, message }) => {
                    Err(Error::new(*kind, message.clone()))
                }
                None => Err(extract_err(e)),
            },
        }
    }

    /// Read data from cache or underlying storage.
    /// Caches the ENTIRE object, then slices to requested range.
    async fn read_range(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        match self.read_full_object().await? {
            Some(buffer) => {
                let rp = Self::full_object_rp(&buffer);
                let buffer = Self::slice_full_object(&buffer, range);
                Ok((rp, buffer))
            }
            None => self.fallback_read(range).await,
        }
    }
}

impl oio::Read for FullReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        match self.read_full_object().await? {
            Some(buffer) => {
                let rp = Self::full_object_rp(&buffer);
                let buffer = Self::slice_full_object(&buffer, range);
                Ok((rp, Box::new(buffer) as Box<dyn oio::ReadStreamDyn>))
            }
            None => self.fallback_open(range).await,
        }
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        self.read_range(range).await
    }
}
