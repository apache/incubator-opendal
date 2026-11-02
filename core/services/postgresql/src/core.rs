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

use asyncband::once::OnceCell;
use sqlx::PgPool;
use sqlx::postgres::PgConnectOptions;

use opendal_core::*;

#[derive(Clone, Debug)]
pub struct PostgresqlCore {
    pub pool: OnceCell<PgPool>,
    pub config: PgConnectOptions,

    pub table: String,
    pub key_field: String,
    pub value_field: String,
}

impl PostgresqlCore {
    async fn get_client(&self) -> Result<&PgPool> {
        self.pool
            .get_or_try_init(|| async {
                PgPool::connect_with(self.config.clone())
                    .await
                    .map_err(parse_postgres_error)
            })
            .await
    }

    pub async fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let pool = self.get_client().await?;

        // PostgreSQL uses `"` for identifier quote. A quoted identifier is case-sensitive and
        // can contain special characters.
        // Read more https://www.postgresql.org/docs/18/sql-syntax-lexical.html#SQL-SYNTAX-IDENTIFIERS
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
        .map_err(parse_postgres_error)?;

        Ok(value.map(Buffer::from))
    }

    pub async fn get_length(&self, path: &str) -> Result<Option<usize>> {
        let pool = self.get_client().await?;

        let value: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            r#"SELECT OCTET_LENGTH("{}")::BIGINT FROM "{}" WHERE "{}" = $1 LIMIT 1"#,
            self.value_field, self.table, self.key_field
        )))
        .bind(path)
        .fetch_optional(pool)
        .await
        .map_err(parse_postgres_error)?;

        value
            .map(|v| {
                v.try_into().map_err(|err| {
                    Error::new(ErrorKind::Unexpected, "postgresql value length is invalid")
                        .set_source(err)
                })
            })
            .transpose()
    }

    pub async fn set(&self, path: &str, value: Buffer) -> Result<()> {
        let pool = self.get_client().await?;

        let table = &self.table;
        let key_field = &self.key_field;
        let value_field = &self.value_field;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"INSERT INTO "{table}" ("{key_field}", "{value_field}")
                VALUES ($1, $2)
                ON CONFLICT ("{key_field}")
                    DO UPDATE SET "{value_field}" = EXCLUDED."{value_field}""#,
        )))
        .bind(path)
        .bind(value.to_vec())
        .execute(pool)
        .await
        .map_err(parse_postgres_error)?;

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
        .map_err(parse_postgres_error)?;

        Ok(())
    }
}

fn parse_postgres_error(err: sqlx::Error) -> Error {
    Error::new(ErrorKind::Unexpected, "unhandled error from postgresql").set_source(err)
}
