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
pub struct PersyCore {
    pub datafile: String,
    pub segment: String,
    pub index: String,
    pub persy: persy::Persy,
}

impl Debug for PersyCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersyCore")
            .field("path", &self.datafile)
            .field("segment", &self.segment)
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

impl PersyCore {
    pub fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let mut read_id = self
            .persy
            .get::<String, persy::PersyId>(&self.index, &path.to_string())
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("GetIndex")), err))?;
        if let Some(id) = read_id.next() {
            let value = self.persy.read(&self.segment, &id).map_err(|err| {
                parse_error(ErrorContext::new(ServiceOperation("ReadRecord")), err)
            })?;
            return Ok(value.map(Buffer::from));
        }

        Ok(None)
    }

    pub fn set(&self, path: &str, value: Buffer) -> Result<()> {
        let mut tx = self.persy.begin().map_err(|err| {
            parse_error(ErrorContext::new(ServiceOperation("BeginTransaction")), err)
        })?;
        let id = tx
            .insert(&self.segment, &value.to_vec())
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("InsertRecord")), err))?;

        tx.put::<String, persy::PersyId>(&self.index, path.to_string(), id)
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("PutIndex")), err))?;
        let prepared = tx.prepare().map_err(|err| {
            parse_error(
                ErrorContext::new(ServiceOperation("PrepareTransaction")),
                err,
            )
        })?;
        prepared.commit().map_err(|err| {
            parse_error(
                ErrorContext::new(ServiceOperation("CommitTransaction")),
                err,
            )
        })?;

        Ok(())
    }

    pub fn delete(&self, path: &str) -> Result<()> {
        let mut delete_id = self
            .persy
            .get::<String, persy::PersyId>(&self.index, &path.to_string())
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("GetIndex")), err))?;
        if let Some(id) = delete_id.next() {
            // Begin a transaction.
            let mut tx = self.persy.begin().map_err(|err| {
                parse_error(ErrorContext::new(ServiceOperation("BeginTransaction")), err)
            })?;
            // Delete the record.
            tx.delete(&self.segment, &id).map_err(|err| {
                parse_error(ErrorContext::new(ServiceOperation("DeleteRecord")), err)
            })?;
            // Remove the index.
            tx.remove::<String, persy::PersyId>(&self.index, path.to_string(), Some(id))
                .map_err(|err| {
                    parse_error(ErrorContext::new(ServiceOperation("RemoveIndex")), err)
                })?;
            // Commit the tx.
            let prepared = tx.prepare().map_err(|err| {
                parse_error(
                    ErrorContext::new(ServiceOperation("PrepareTransaction")),
                    err,
                )
            })?;
            prepared.commit().map_err(|err| {
                parse_error(
                    ErrorContext::new(ServiceOperation("CommitTransaction")),
                    err,
                )
            })?;
        }

        Ok(())
    }
}

fn parse_error<T: Into<persy::PersyError>>(ctx: ErrorContext, err: persy::PE<T>) -> Error {
    let err: persy::PersyError = err.persy_error();
    let kind = match err {
        persy::PersyError::RecordNotFound(_) => ErrorKind::NotFound,
        _ => ErrorKind::Unexpected,
    };

    Error::new(kind, "error from persy")
        .with_context("service_operation", ctx.service_operation.0)
        .set_source(err)
}
