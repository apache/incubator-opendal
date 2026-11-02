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

use std::io;
use std::sync::Arc;

use super::core::HdfsCore;
use opendal_core::raw::*;
use opendal_core::*;

pub struct HdfsDeleter {
    core: Arc<HdfsCore>,
}

impl HdfsDeleter {
    pub fn new(core: Arc<HdfsCore>) -> Self {
        Self { core }
    }
}

impl oio::OneShotDelete for HdfsDeleter {
    async fn delete_once(&self, path: String, args: OpDelete) -> Result<()> {
        let p = build_rooted_abs_path(&self.core.root, &path);

        let meta = self.core.client.metadata(&p);

        if let Err(err) = meta {
            return if err.kind() == io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(new_std_io_error(err))
            };
        }

        // Safety: Err branch has been checked, it's OK to unwrap.
        let meta = meta.ok().unwrap();

        let result = if meta.is_dir() {
            if args.recursive() {
                self.core.client.remove_dir_all(&p)
            } else {
                self.core.client.remove_dir(&p)
            }
        } else {
            self.core.client.remove_file(&p)
        };

        result.map_err(new_std_io_error)?;

        Ok(())
    }
}
