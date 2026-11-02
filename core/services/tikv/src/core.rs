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

use asyncband::once::OnceCell;
use tikv_client::Config;
use tikv_client::RawClient;

use super::TIKV_SCHEME;
use opendal_core::raw::ServiceOperation;
use opendal_core::*;

#[derive(Clone, Copy, Debug)]
struct ErrorContext {
    service_operation: ServiceOperation,
}

impl ErrorContext {
    const fn new(service_operation: ServiceOperation) -> Self {
        Self { service_operation }
    }
}

/// TikvCore holds the configuration and client for interacting with TiKV.
#[derive(Clone)]
pub struct TikvCore {
    pub client: OnceCell<RawClient>,
    pub endpoints: Vec<String>,
    pub insecure: bool,
    pub ca_path: Option<String>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

impl Debug for TikvCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TikvCore")
            .field("endpoints", &self.endpoints)
            .field("insecure", &self.insecure)
            .finish()
    }
}

impl TikvCore {
    async fn get_connection(&self) -> Result<&RawClient> {
        self.client
            .get_or_try_init(|| async {
                if self.insecure {
                    return RawClient::new(self.endpoints.clone()).await.map_err(|err| {
                        parse_error(ErrorContext::new(ServiceOperation("Connect")), err)
                    });
                }

                if let Some(ca_path) = self.ca_path.as_ref()
                    && let Some(key_path) = self.key_path.as_ref()
                    && let Some(cert_path) = self.cert_path.as_ref()
                {
                    let config = Config::default().with_security(ca_path, cert_path, key_path);
                    return RawClient::new_with_config(self.endpoints.clone(), config)
                        .await
                        .map_err(|err| {
                            parse_error(ErrorContext::new(ServiceOperation("Connect")), err)
                        });
                }

                Err(
                    Error::new(ErrorKind::ConfigInvalid, "invalid configuration")
                        .with_context("service", TIKV_SCHEME)
                        .with_context("endpoints", format!("{:?}", self.endpoints)),
                )
            })
            .await
    }

    pub async fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let result = self
            .get_connection()
            .await?
            .get(path.to_owned())
            .await
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("Get")), err))?;
        Ok(result.map(Buffer::from))
    }

    pub async fn set(&self, path: &str, value: Buffer) -> Result<()> {
        self.get_connection()
            .await?
            .put(path.to_owned(), value.to_vec())
            .await
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("Put")), err))
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        self.get_connection()
            .await?
            .delete(path.to_owned())
            .await
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("Delete")), err))
    }
}

fn parse_error(ctx: ErrorContext, err: tikv_client::Error) -> Error {
    let (kind, message) = if ctx.service_operation.0 == "Connect" {
        (ErrorKind::ConfigInvalid, "invalid configuration")
    } else {
        (ErrorKind::Unexpected, "error from tikv")
    };

    Error::new(kind, message)
        .with_context("service", TIKV_SCHEME)
        .with_context("service_operation", ctx.service_operation.0)
        .set_source(err)
}
