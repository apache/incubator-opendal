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

use futures::{StreamExt, stream::BoxStream};
use object_store::path::Path as ObjectStorePath;
use object_store::{ObjectMeta, ObjectStore};
use opendal::raw::*;
use opendal::*;

use super::error::parse_error;
use crate::service::core::format_metadata;

pub struct ObjectStoreLister {
    stream: BoxStream<'static, object_store::Result<ObjectMeta>>,
}

impl ObjectStoreLister {
    pub(crate) fn new(
        store: Arc<dyn ObjectStore + 'static>,
        path: &str,
        args: OpList,
    ) -> Result<Self> {
        // If start_after is specified, use list_with_offset
        let mut stream = if let Some(start_after) = args.start_after() {
            store
                .list_with_offset(
                    Some(&ObjectStorePath::from(path)),
                    &ObjectStorePath::from(start_after),
                )
                .boxed()
        } else {
            store.list(Some(&ObjectStorePath::from(path))).boxed()
        };

        // Process listing arguments
        if let Some(limit) = args.limit() {
            stream = stream.take(limit).boxed();
        }

        Ok(Self { stream })
    }
}

// ObjectStoreLister is safe to share between threads, because it only has &mut self API calls.
unsafe impl Sync for ObjectStoreLister {}

impl oio::List for ObjectStoreLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        match self.stream.next().await {
            Some(Ok(meta)) => {
                let metadata = format_metadata(&meta);
                let entry = oio::Entry::new(meta.location.as_ref(), metadata.clone());
                Ok(Some(entry))
            }
            Some(Err(e)) => Err(parse_error(e)),
            None => Ok(None),
        }
    }
}
