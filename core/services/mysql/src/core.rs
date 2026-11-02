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
use sqlx::MySqlPool;
use sqlx::mysql::MySqlConnectOptions;

use opendal_core::*;

#[derive(Clone, Debug)]
pub struct MysqlCore {
    pub pool: OnceCell<MySqlPool>,
    pub config: MySqlConnectOptions,

    pub table: String,
    pub key_field: String,
    pub value_field: String,
}

impl MysqlCore {
    async fn get_client(&self) -> Result<&MySqlPool> {
        self.pool
            .get_or_try_init(|| async {
                MySqlPool::connect_with(self.config.clone())
                    .await
                    .map_err(parse_mysql_error)
            })
            .await
    }

    pub async fn get(&self, path: &str) -> Result<Option<Buffer>> {
        let pool = self.get_client().await?;

        // MySQL uses a backtick for identifier quote. An identifier may or may not be case-sensitive,
        // depending on database configuration. Only a quoted identifier can have special characters.
        // Read more:
        // https://dev.mysql.com/doc/refman/9.7/en/identifiers.html
        // https://dev.mysql.com/doc/refman/9.7/en/identifier-case-sensitivity.html
        //
        // We use:
        // - formatted, quoted identifiers for trusted table and field configuration
        // - bind parameters for values to avoid malformed SQL and SQL injection
        // to ensure correctness. `AssertSqlSafe` records this audit for sqlx.
        let value: Option<Vec<u8>> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT `{}` FROM `{}` WHERE `{}` = ? LIMIT 1",
            self.value_field, self.table, self.key_field
        )))
        .bind(path)
        .fetch_optional(pool)
        .await
        .map_err(parse_mysql_error)?;

        Ok(value.map(Buffer::from))
    }

    pub async fn get_length(&self, path: &str) -> Result<Option<usize>> {
        let pool = self.get_client().await?;

        let value: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT OCTET_LENGTH(`{}`) FROM `{}` WHERE `{}` = ? LIMIT 1",
            self.value_field, self.table, self.key_field
        )))
        .bind(path)
        .fetch_optional(pool)
        .await
        .map_err(parse_mysql_error)?;

        value
            .map(|v| {
                v.try_into().map_err(|err| {
                    Error::new(ErrorKind::Unexpected, "mysql value length is invalid")
                        .set_source(err)
                })
            })
            .transpose()
    }

    pub async fn set(&self, path: &str, value: Buffer) -> Result<()> {
        let pool = self.get_client().await?;

        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"INSERT INTO `{}` (`{}`, `{}`) VALUES (?, ?)
            ON DUPLICATE KEY UPDATE `{}` = VALUES(`{}`)"#,
            self.table, self.key_field, self.value_field, self.value_field, self.value_field
        )))
        .bind(path)
        .bind(value.to_vec())
        .execute(pool)
        .await
        .map_err(parse_mysql_error)?;

        Ok(())
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let pool = self.get_client().await?;

        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM `{}` WHERE `{}` = ?",
            self.table, self.key_field
        )))
        .bind(path)
        .execute(pool)
        .await
        .map_err(parse_mysql_error)?;

        Ok(())
    }

    pub async fn list(&self, path: &str) -> Result<Vec<String>> {
        let pool = self.get_client().await?;

        let mut sql = format!(
            "SELECT `{}` FROM `{}` WHERE `{}` LIKE ?",
            self.key_field, self.table, self.key_field
        );
        sql.push_str(&format!(" ORDER BY `{}`", self.key_field));

        let escaped = escape_like(path);
        sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(format!("{escaped}%"))
            .fetch_all(pool)
            .await
            .map_err(parse_mysql_error)
    }
}

fn escape_like(s: &str) -> String {
    const ESCAPE_CHAR: char = '\\';
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            c if c == ESCAPE_CHAR => {
                out.push(ESCAPE_CHAR);
                out.push(ESCAPE_CHAR);
            }
            '%' | '_' => {
                out.push(ESCAPE_CHAR);
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn parse_mysql_error(err: sqlx::Error) -> Error {
    Error::new(ErrorKind::Unexpected, "unhandled error from mysql").set_source(err)
}

#[cfg(test)]
mod tests {
    use crate::core::escape_like;

    #[test]
    fn test_escape_like_basic() {
        assert_eq!(escape_like("abc"), "abc");
        assert_eq!(escape_like("foo"), "foo");
    }

    #[test]
    fn test_escape_like_wildcards() {
        assert_eq!(escape_like("%"), r"\%");
        assert_eq!(escape_like("_"), r"\_");
        assert_eq!(escape_like("a%b_c"), r"a\%b\_c");
    }

    #[test]
    fn test_escape_like_escape_char() {
        assert_eq!(escape_like(r"\"), r"\\");
        assert_eq!(escape_like(r"\%"), r"\\\%");
    }

    #[test]
    fn test_escape_like_mixed() {
        let input = r"foo\%bar_baz%";
        let expected = r"foo\\\%bar\_baz\%";

        assert_eq!(escape_like(input), expected);
    }
}
