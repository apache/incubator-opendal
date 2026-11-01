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

use opendal_core::*;
use redb::ReadableDatabase;

#[derive(Clone)]
pub struct RedbCore {
    pub db: Arc<redb::Database>,
    pub datadir: Option<String>,
    pub table: String,
}

impl Debug for RedbCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedbCore")
            .field("path", &self.datadir)
            .field("table", &self.table)
            .finish_non_exhaustive()
    }
}

impl RedbCore {
    pub fn scan(&self, prefix: &str) -> Result<redb::Range<'static, &'static str, &'static [u8]>> {
        let read_txn = self.db.begin_read().map_err(parse_transaction_error)?;
        let table_define: redb::TableDefinition<&str, &[u8]> =
            redb::TableDefinition::new(&self.table);
        let table = read_txn
            .open_table(table_define)
            .map_err(parse_table_error)?;
        table.range(prefix..).map_err(parse_storage_error)
    }

    pub fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let read_txn = self.db.begin_read().map_err(parse_transaction_error)?;

        let table_define: redb::TableDefinition<&str, &[u8]> =
            redb::TableDefinition::new(&self.table);

        let table = read_txn
            .open_table(table_define)
            .map_err(parse_table_error)?;

        let result = match table.get(path) {
            Ok(Some(v)) => Ok(Some(v.value().to_vec())),
            Ok(None) => Ok(None),
            Err(e) => Err(parse_storage_error(e)),
        }?;
        Ok(result.map(Buffer::from))
    }

    pub fn set(&self, path: &str, value: Buffer) -> Result<()> {
        let write_txn = self.db.begin_write().map_err(parse_transaction_error)?;

        let table_define: redb::TableDefinition<&str, &[u8]> =
            redb::TableDefinition::new(&self.table);

        {
            let mut table = write_txn
                .open_table(table_define)
                .map_err(parse_table_error)?;

            table
                .insert(path, &*value.to_vec())
                .map_err(parse_storage_error)?;
        }

        write_txn.commit().map_err(parse_commit_error)?;
        Ok(())
    }

    pub fn delete(&self, path: &str) -> Result<()> {
        let write_txn = self.db.begin_write().map_err(parse_transaction_error)?;

        let table_define: redb::TableDefinition<&str, &[u8]> =
            redb::TableDefinition::new(&self.table);

        {
            let mut table = write_txn
                .open_table(table_define)
                .map_err(parse_table_error)?;

            table.remove(path).map_err(parse_storage_error)?;
        }

        write_txn.commit().map_err(parse_commit_error)?;
        Ok(())
    }
}

fn parse_transaction_error(e: redb::TransactionError) -> Error {
    Error::new(ErrorKind::Unexpected, "error from redb").set_source(e)
}

fn parse_table_error(e: redb::TableError) -> Error {
    match e {
        redb::TableError::TableDoesNotExist(_) => {
            Error::new(ErrorKind::NotFound, "error from redb").set_source(e)
        }
        _ => Error::new(ErrorKind::Unexpected, "error from redb").set_source(e),
    }
}

pub(super) fn parse_storage_error(e: redb::StorageError) -> Error {
    Error::new(ErrorKind::Unexpected, "error from redb").set_source(e)
}

fn parse_commit_error(e: redb::CommitError) -> Error {
    Error::new(ErrorKind::Unexpected, "error from redb").set_source(e)
}

pub fn parse_database_error(e: redb::DatabaseError) -> Error {
    Error::new(ErrorKind::Unexpected, "error from redb").set_source(e)
}

/// Check if a table exists, otherwise create it.
pub fn create_table(db: &redb::Database, table: &str) -> Result<()> {
    // Only one `WriteTransaction` is permitted at same time,
    // applying new one will block until it available.
    //
    // So we first try checking table existence via `ReadTransaction`.
    {
        let read_txn = db.begin_read().map_err(parse_transaction_error)?;

        let table_define: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new(table);

        match read_txn.open_table(table_define) {
            Ok(_) => return Ok(()),
            Err(redb::TableError::TableDoesNotExist(_)) => (),
            Err(e) => return Err(parse_table_error(e)),
        }
    }

    {
        let write_txn = db.begin_write().map_err(parse_transaction_error)?;

        let table_define: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new(table);

        write_txn
            .open_table(table_define)
            .map_err(parse_table_error)?;
        write_txn.commit().map_err(parse_commit_error)?;
    }

    Ok(())
}
