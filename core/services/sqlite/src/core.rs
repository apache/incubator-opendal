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
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;

use super::backend::parse_sqlite_error;
use opendal_core::*;

#[derive(Debug, Clone)]
pub struct SqliteCore {
    pub pool: OnceCell<SqlitePool>,
    pub config: SqliteConnectOptions,

    pub table: String,
    pub key_field: String,
    pub value_field: String,
}

impl SqliteCore {
    pub async fn get_client(&self) -> Result<&SqlitePool> {
        self.pool
            .get_or_try_init(|| async {
                SqlitePool::connect_with(self.config.clone())
                    .await
                    .map_err(parse_sqlite_error)
            })
            .await
    }

    pub async fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let pool = self.get_client().await?;

        // SQLite prefers using standard `"` for identifier quotes instead of
        // MySQL-compatible backtick ` or [] quotes.
        // Identifier quotes are case-sensitive and allow special characters.
        // Read more https://www.sqlite.org/quirks.html#double_quoted_string_literals_are_accepted
        //
        // We use:
        // - formatted, quoted identifiers for trusted table and field configuration
        // - bind parameters for values to avoid malformed SQL and SQL injection
        // to ensure correctness. `AssertSqlSafe` records this audit for sqlx.
        let value: Option<Vec<u8>> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            r#"SELECT "{}" FROM "{}" WHERE "{}" = $1 LIMIT 1"#,
            self.value_field, self.table, self.key_field
        )))
        .bind(path)
        .fetch_optional(pool)
        .await
        .map_err(parse_sqlite_error)?;

        Ok(value.map(Buffer::from))
    }

    pub async fn get_length(&self, path: &str) -> Result<Option<usize>> {
        let pool = self.get_client().await?;

        let value: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            r#"SELECT LENGTH(CAST("{}" AS BLOB)) FROM "{}" WHERE "{}" = $1 LIMIT 1"#,
            self.value_field, self.table, self.key_field
        )))
        .bind(path)
        .fetch_optional(pool)
        .await
        .map_err(parse_sqlite_error)?;

        value
            .map(|v| {
                v.try_into().map_err(|err| {
                    Error::new(ErrorKind::Unexpected, "sqlite value length is invalid")
                        .set_source(err)
                })
            })
            .transpose()
    }

    pub async fn count_under(&self, path: &str) -> Result<i64> {
        let pool = self.get_client().await?;

        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            r#"SELECT COUNT(*) FROM "{}" WHERE instr("{}", $1) = 1 LIMIT 1"#,
            self.table, self.key_field
        )))
        .bind(path.to_string())
        .fetch_one(pool)
        .await
        .map_err(parse_sqlite_error)
    }

    pub async fn get_range(
        &self,
        path: &str,
        start: isize,
        limit: Option<isize>,
    ) -> Result<Option<(Buffer, u64)>> {
        let pool = self.get_client().await?;
        if start < 0 || limit.is_some_and(|v| v < 0) {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "sqlite range contains negative value",
            ));
        }

        let start = start.checked_add(1).ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "sqlite range start exceeds supported value",
            )
        })?;
        let start: i64 = start.try_into().map_err(|err| {
            Error::new(
                ErrorKind::Unexpected,
                "sqlite range start exceeds supported value",
            )
            .set_source(err)
        })?;
        let value = match limit {
            Some(limit) => {
                let limit: i64 = limit.try_into().map_err(|err| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "sqlite range size exceeds supported value",
                    )
                    .set_source(err)
                })?;
                sqlx::query_as(sqlx::AssertSqlSafe(format!(
                    r#"SELECT SUBSTR(CAST("{}" AS BLOB), $1, $2), LENGTH(CAST("{}" AS BLOB)) FROM "{}" WHERE "{}" = $3 LIMIT 1"#,
                    self.value_field, self.value_field, self.table, self.key_field
                )))
                .bind(start)
                .bind(limit)
                .bind(path)
                .fetch_optional(pool)
                .await
            }
            None => {
                sqlx::query_as(sqlx::AssertSqlSafe(format!(
                    r#"SELECT SUBSTR(CAST("{}" AS BLOB), $1), LENGTH(CAST("{}" AS BLOB)) FROM "{}" WHERE "{}" = $2 LIMIT 1"#,
                    self.value_field, self.value_field, self.table, self.key_field
                )))
                .bind(start)
                .bind(path)
                .fetch_optional(pool)
                .await
            }
        };
        let value: Option<(Vec<u8>, i64)> = value.map_err(parse_sqlite_error)?;

        Ok(value.map(|(bs, size)| (Buffer::from(bs), size as u64)))
    }

    pub async fn set(&self, path: &str, value: Buffer) -> Result<()> {
        let pool = self.get_client().await?;

        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"INSERT OR REPLACE INTO "{}" ("{}", "{}") VALUES ($1, $2)"#,
            self.table, self.key_field, self.value_field,
        )))
        .bind(path)
        .bind(value.to_vec())
        .execute(pool)
        .await
        .map_err(parse_sqlite_error)?;

        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let pool = self.get_client().await?;

        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"DELETE FROM "{}" WHERE "{}" = $1"#,
            self.table, self.key_field
        )))
        .bind(path)
        .execute(pool)
        .await
        .map_err(parse_sqlite_error)?;

        Ok(())
    }
}
