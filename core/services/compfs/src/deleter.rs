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

use super::core::*;
use opendal_core::raw::*;
use opendal_core::*;

pub struct CompfsDeleter {
    core: Arc<CompfsCore>,
}

impl CompfsDeleter {
    pub fn new(core: Arc<CompfsCore>) -> Self {
        Self { core }
    }
}

impl oio::OneShotDelete for CompfsDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let res = if path.ends_with('/') {
            let path = self.core.prepare_path(&path)?;
            self.core
                .exec(move || async move { compio::fs::remove_dir(path).await })
                .await
        } else {
            let path = self.core.prepare_path(&path)?;
            self.core
                .exec(move || async move { compio::fs::remove_file(path).await })
                .await
        };
        match res {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}
