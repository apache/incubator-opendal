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
use std::sync::Arc;

use foundationdb::Database;
use foundationdb::api::NetworkAutoStop;

use super::FOUNDATIONDB_SCHEME;
use opendal_core::*;

#[derive(Clone)]
pub struct FoundationdbCore {
    pub db: Arc<Database>,
    pub _network: Arc<NetworkAutoStop>,
}

impl Debug for FoundationdbCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FoundationdbCore").finish_non_exhaustive()
    }
}

impl FoundationdbCore {
    pub async fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let transaction = self.db.create_trx().expect("Unable to create transaction");

        match transaction.get(path.as_bytes(), false).await {
            Ok(slice) => match slice {
                Some(data) => Ok(Some(Buffer::from(data.to_vec()))),
                None => Err(Error::new(
                    ErrorKind::NotFound,
                    "foundationdb: key not found",
                )),
            },
            Err(_) => Err(Error::new(
                ErrorKind::NotFound,
                "foundationdb: key not found",
            )),
        }
    }

    pub async fn set(&self, path: &str, value: Buffer) -> Result<()> {
        let transaction = self.db.create_trx().expect("Unable to create transaction");

        transaction.set(path.as_bytes(), &value.to_vec());

        match transaction.commit().await {
            Ok(_) => Ok(()),
            Err(e) => Err(parse_transaction_commit_error(e)),
        }
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let transaction = self.db.create_trx().expect("Unable to create transaction");
        transaction.clear(path.as_bytes());

        match transaction.commit().await {
            Ok(_) => Ok(()),
            Err(e) => Err(parse_transaction_commit_error(e)),
        }
    }
}

fn parse_transaction_commit_error(e: foundationdb::TransactionCommitError) -> Error {
    Error::new(ErrorKind::Unexpected, e.to_string().as_str())
        .with_context("service", FOUNDATIONDB_SCHEME)
}
