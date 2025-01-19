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

use http::header;
use http::Request;
use http::StatusCode;

use super::core::SeafileCore;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub type SeafileWriters = oio::OneShotWriter<SeafileWriter>;

pub struct SeafileWriter {
    core: Arc<SeafileCore>,
    _op: OpWrite,
    path: String,
}

impl SeafileWriter {
    pub fn new(core: Arc<SeafileCore>, op: OpWrite, path: String) -> Self {
        SeafileWriter {
            core,
            _op: op,
            path,
        }
    }
}

impl oio::OneShotWrite for SeafileWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let upload_url = self.core.get_upload_url().await?;

        let req = Request::post(upload_url);

        let (filename, relative_path) = if self.path.ends_with('/') {
            ("", build_abs_path(&self.core.root, &self.path))
        } else {
            let (filename, relative_path) = (get_basename(&self.path), get_parent(&self.path));
            (filename, build_abs_path(&self.core.root, relative_path))
        };

        let file_part = FormDataPart::new("file")
            .header(
                header::CONTENT_DISPOSITION,
                format!("form-data; name=\"file\"; filename=\"{filename}\"")
                    .parse()
                    .unwrap(),
            )
            .content(bs);

        let multipart = Multipart::new()
            .part(FormDataPart::new("parent_dir").content("/"))
            .part(FormDataPart::new("relative_path").content(relative_path.clone()))
            .part(FormDataPart::new("replace").content("1"))
            .part(file_part);

        let req = multipart.apply(req)?;

        let resp = self.core.send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(Metadata::default()),
            _ => Err(parse_error(resp)),
        }
    }
}
