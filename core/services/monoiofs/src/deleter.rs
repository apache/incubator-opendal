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

use opendal_core::raw::*;
use opendal_core::*;

use super::core::MonoiofsCore;

pub struct MonoiofsDeleter {
    core: Arc<MonoiofsCore>,
}

impl MonoiofsDeleter {
    pub fn new(core: Arc<MonoiofsCore>) -> Self {
        Self { core }
    }
}

impl oio::OneShotDelete for MonoiofsDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let path = self.core.prepare_path(&path)?;
        let meta = self
            .core
            .dispatch({
                let path = path.clone();
                move || monoio::fs::metadata(path)
            })
            .await;
        match meta {
            Ok(meta) => {
                if meta.is_dir() {
                    self.core
                        .dispatch(move || monoio::fs::remove_dir(path))
                        .await
                        .map_err(new_std_io_error)?;
                } else {
                    self.core
                        .dispatch(move || monoio::fs::remove_file(path))
                        .await
                        .map_err(new_std_io_error)?;
                }

                Ok(())
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(new_std_io_error(err)),
        }
    }
}
