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

use std::collections::VecDeque;
use std::sync::Arc;

use opendal_core::raw::*;
use opendal_core::*;

use crate::core::{ErrorContext, GcsGrpcCore, parse_object, parse_status};
use crate::generated::google::storage::v2::ListObjectsRequest;

pub(super) struct GcsGrpcLister {
    core: Arc<GcsGrpcCore>,
    ctx: OperationContext,
    prefix: String,
    delimiter: String,
    page_size: i32,
    page_token: String,
    start_after: String,
    entries: VecDeque<oio::Entry>,
    done: bool,
}

impl GcsGrpcLister {
    pub(super) fn new(
        core: Arc<GcsGrpcCore>,
        ctx: OperationContext,
        path: &str,
        args: OpList,
    ) -> Self {
        let prefix = core.object_name(path);
        let start_after = args
            .start_after()
            .map(|path| core.object_name(path))
            .unwrap_or_default();
        Self {
            core,
            ctx,
            prefix,
            delimiter: if args.recursive() { "" } else { "/" }.to_string(),
            page_size: args.limit().unwrap_or(1000).min(1000) as i32,
            page_token: String::new(),
            start_after,
            entries: VecDeque::new(),
            done: false,
        }
    }

    async fn fetch_page(&mut self) -> Result<()> {
        let request = ListObjectsRequest {
            parent: self.core.bucket_resource(),
            page_size: self.page_size,
            page_token: self.page_token.clone(),
            delimiter: self.delimiter.clone(),
            prefix: self.prefix.clone(),
            lexicographic_start: self.start_after.clone(),
        };
        let request = self
            .core
            .request(
                &self.ctx,
                request,
                &[("bucket", &self.core.bucket_resource())],
            )
            .await?;
        let response = self
            .core
            .client()
            .list_objects(request)
            .await
            .map_err(|status| {
                parse_status(ErrorContext::new(ServiceOperation("ListObjects")), status)
            })?
            .into_inner();
        self.page_token = response.next_page_token;
        self.done = self.page_token.is_empty();

        for object in response.objects {
            if object.name == self.start_after {
                continue;
            }
            let path = build_rel_path(&self.core.root, &object.name);
            self.entries
                .push_back(oio::Entry::with(path, parse_object(&object)));
        }
        for prefix in response.prefixes {
            if prefix == self.start_after {
                continue;
            }
            let path = build_rel_path(&self.core.root, &prefix);
            self.entries
                .push_back(oio::Entry::with(path, MetadataBuilder::dir().build()));
        }
        Ok(())
    }
}

impl oio::List for GcsGrpcLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        loop {
            if let Some(entry) = self.entries.pop_front() {
                return Ok(Some(entry));
            }
            if self.done {
                return Ok(None);
            }
            self.fetch_page().await?;
        }
    }
}
