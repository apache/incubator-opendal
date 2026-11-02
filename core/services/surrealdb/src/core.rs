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

use asyncband::once::OnceCell;
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Database;

use opendal_core::*;

#[derive(Clone)]
pub struct SurrealdbCore {
    pub db: OnceCell<Arc<Surreal<Any>>>,
    pub connection_string: String,

    pub username: String,
    pub password: String,
    pub namespace: String,
    pub database: String,

    pub table: String,
    pub key_field: String,
    pub value_field: String,
}

impl Debug for SurrealdbCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurrealdbCore")
            .field("connection_string", &self.connection_string)
            .field("username", &self.username)
            .field("namespace", &self.namespace)
            .field("database", &self.database)
            .field("table", &self.table)
            .field("key_field", &self.key_field)
            .field("value_field", &self.value_field)
            .finish_non_exhaustive()
    }
}

impl SurrealdbCore {
    async fn get_connection(&self) -> Result<&Surreal<Any>> {
        self.db
            .get_or_try_init(|| async {
                let namespace = self.namespace.as_str();
                let database = self.database.as_str();

                let db: Surreal<Any> = Surreal::init();
                db.connect(self.connection_string.clone())
                    .await
                    .map_err(parse_surrealdb_error)?;

                if !self.username.is_empty() && !self.password.is_empty() {
                    db.signin(Database {
                        namespace: namespace.to_string(),
                        database: database.to_string(),
                        username: self.username.clone(),
                        password: self.password.clone(),
                    })
                    .await
                    .map_err(parse_surrealdb_error)?;
                }
                db.use_ns(namespace)
                    .use_db(database)
                    .await
                    .map_err(parse_surrealdb_error)?;

                Ok(Arc::new(db))
            })
            .await
            .map(|v| v.as_ref())
    }

    pub async fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let query: String = if self.key_field == "id" {
            "SELECT type::field($value_field) FROM type::thing($table, $path)".to_string()
        } else {
            format!(
                "SELECT type::field($value_field) FROM type::table($table) WHERE {} = $path LIMIT 1",
                self.key_field
            )
        };

        let mut result = self
            .get_connection()
            .await?
            .query(query)
            .bind(("namespace", "opendal"))
            .bind(("path", path.to_string()))
            .bind(("table", self.table.to_string()))
            .bind(("value_field", self.value_field.to_string()))
            .await
            .map_err(parse_surrealdb_error)?;

        let value: Option<Vec<u8>> = result
            .take((0, self.value_field.as_str()))
            .map_err(parse_surrealdb_error)?;

        Ok(value.map(Buffer::from))
    }

    pub async fn get_length(&self, path: &str) -> Result<Option<usize>> {
        let query: String = if self.key_field == "id" {
            "SELECT bytes::len(type::field($value_field)) AS content_length FROM type::thing($table, $path)"
                .to_string()
        } else {
            format!(
                "SELECT bytes::len(type::field($value_field)) AS content_length FROM type::table($table) WHERE {} = $path LIMIT 1",
                self.key_field
            )
        };

        let mut result = self
            .get_connection()
            .await?
            .query(query)
            .bind(("namespace", "opendal"))
            .bind(("path", path.to_string()))
            .bind(("table", self.table.to_string()))
            .bind(("value_field", self.value_field.to_string()))
            .await
            .map_err(parse_surrealdb_error)?;

        let value: Option<i64> = result
            .take((0, "content_length"))
            .map_err(parse_surrealdb_error)?;

        value
            .map(|v| {
                v.try_into().map_err(|err| {
                    Error::new(ErrorKind::Unexpected, "surrealdb value length is invalid")
                        .set_source(err)
                })
            })
            .transpose()
    }

    pub async fn set(&self, path: &str, value: Buffer) -> Result<()> {
        let query = format!(
            "INSERT INTO {} ({}, {}) \
            VALUES ($path, $value) \
            ON DUPLICATE KEY UPDATE {} = $value",
            self.table, self.key_field, self.value_field, self.value_field
        );
        self.get_connection()
            .await?
            .query(query)
            .bind(("path", path.to_string()))
            .bind(("value", value.to_vec()))
            .await
            .map_err(parse_surrealdb_error)?;
        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let query: String = if self.key_field == "id" {
            "DELETE FROM type::thing($table, $path)".to_string()
        } else {
            format!(
                "DELETE FROM type::table($table) WHERE {} = $path",
                self.key_field
            )
        };

        self.get_connection()
            .await?
            .query(query.as_str())
            .bind(("path", path.to_string()))
            .bind(("table", self.table.to_string()))
            .await
            .map_err(parse_surrealdb_error)?;
        Ok(())
    }
}

fn parse_surrealdb_error(err: surrealdb::Error) -> Error {
    Error::new(ErrorKind::Unexpected, "unhandled error from surrealdb").set_source(err)
}
