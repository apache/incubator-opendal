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

use std::fmt::Debug;

use opendal_core::raw::ServiceOperation;
use opendal_core::*;

/// Context needed to classify an error from this service.
#[derive(Clone, Copy, Debug)]
struct ErrorContext {
    service_operation: ServiceOperation,
}

impl ErrorContext {
    const fn new(service_operation: ServiceOperation) -> Self {
        Self { service_operation }
    }
}

#[derive(Clone)]
pub struct SledCore {
    pub datadir: String,
    pub tree: sled::Tree,
}

impl Debug for SledCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SledCore")
            .field("path", &self.datadir)
            .finish()
    }
}

impl SledCore {
    pub fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let res = self
            .tree
            .get(path)
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("Get")), err))?;
        Ok(res.map(|v| Buffer::from(v.to_vec())))
    }

    pub fn set(&self, path: &str, value: Buffer) -> Result<()> {
        self.tree
            .insert(path, value.to_vec())
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("Insert")), err))?;
        Ok(())
    }

    pub fn delete(&self, path: &str) -> Result<()> {
        self.tree
            .remove(path)
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("Remove")), err))?;
        Ok(())
    }

    pub fn list(&self, path: &str) -> Result<Vec<(String, u64)>> {
        let it = self.tree.scan_prefix(path);
        let mut res = Vec::default();

        for i in it {
            let (key, value) = i.map_err(|err| {
                parse_error(ErrorContext::new(ServiceOperation("ScanPrefix")), err)
            })?;
            let bs = key.to_vec();
            let v = String::from_utf8(bs).map_err(|err| {
                Error::new(ErrorKind::Unexpected, "store key is not valid utf-8 string")
                    .set_source(err)
            })?;

            res.push((v, value.len() as u64));
        }

        Ok(res)
    }
}

fn parse_error(ctx: ErrorContext, err: sled::Error) -> Error {
    Error::new(ErrorKind::Unexpected, "error from sled")
        .with_context("service_operation", ctx.service_operation.0)
        .set_source(err)
}
