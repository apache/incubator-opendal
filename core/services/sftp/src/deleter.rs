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

use super::core::SftpCore;
use super::core::is_not_found;
use super::core::parse_sftp_error;
use opendal_core::raw::*;
use opendal_core::*;

pub struct SftpDeleter {
    core: Arc<SftpCore>,
}

impl SftpDeleter {
    pub fn new(core: Arc<SftpCore>) -> Self {
        Self { core }
    }
}

impl oio::OneShotDelete for SftpDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let client = self.core.connect().await?;

        let mut fs = client.fs();
        fs.set_cwd(&self.core.root);

        let res = if path.ends_with('/') {
            fs.remove_dir(path).await
        } else {
            fs.remove_file(path).await
        };

        match res {
            Ok(()) => Ok(()),
            Err(e) if is_not_found(&e) => Ok(()),
            Err(e) => Err(parse_sftp_error(e)),
        }
    }
}
